use std::collections::VecDeque;
use std::path::{Component, Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use bevy::prelude::*;
use bevy::tasks::{IoTaskPool, Task, futures::check_ready};
use mcrs_minecraft_client_world::session::{LocalWorldOrigin, OpenLocalWorld};
use mcrs_minecraft_client_world::world_list::{
    CreateWorld, DeleteWorld, PlayWorld, ReadWorldList, WorldList, WorldStoreError,
    WorldStoreFailed, WorldTypes,
};
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_registries::Registries;
use mcrs_minecraft_save::{
    NewWorld, SaveLock, WorldSummary, create_world, delete_world, read_level_dat, read_saves,
    restore_level_dat_backup,
};

use crate::local_server::{LocalServerPlugin, data_pack_registries};

#[derive(Clone)]
pub struct WorldListPlugin {
    pub saves: PathBuf,
    pub server: LocalServerPlugin,
}

#[derive(Resource, Default)]
enum DataPackRegistries {
    #[default]
    Unloaded,
    Loading,
    Ready(Registries),
    Failed(WorldStoreError),
}

#[derive(Resource)]
struct WorldStore {
    plugin: WorldListPlugin,
    pending: VecDeque<Job>,
    running: Option<Task<Done>>,
}

enum Job {
    Read,
    LoadRegistries,
    Create(NewWorld),
    Delete(String),
    Open(String),
}

struct LockedWorld {
    lock: SaveLock,
    host: Option<Uuid>,
}

enum Done {
    Read(Result<Vec<WorldSummary>, WorldStoreError>),
    Registries(Result<Registries, WorldStoreError>),
    Created(Result<SaveLock, WorldStoreError>),
    Opened {
        folder_id: String,
        world: Result<LockedWorld, WorldStoreError>,
    },
    Deleted {
        folder_id: String,
        removed: Result<(), WorldStoreError>,
        list: Result<Vec<WorldSummary>, WorldStoreError>,
    },
}

impl Plugin for WorldListPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(WorldStore {
            plugin: self.clone(),
            pending: VecDeque::new(),
            running: None,
        })
        .init_resource::<DataPackRegistries>()
        .init_resource::<WorldList>()
        .init_resource::<WorldTypes>()
        .add_message::<ReadWorldList>()
        .add_message::<CreateWorld>()
        .add_message::<DeleteWorld>()
        .add_message::<PlayWorld>()
        .add_message::<WorldStoreFailed>()
        .add_message::<OpenLocalWorld>()
        .add_systems(
            Update,
            (
                queue_jobs.run_if(
                    on_message::<ReadWorldList>
                        .or_else(on_message::<CreateWorld>)
                        .or_else(on_message::<DeleteWorld>),
                ),
                finish_job.run_if(running),
                start_job.run_if(not(running).and_then(pending)),
                play_world.run_if(on_message::<PlayWorld>),
            )
                .chain(),
        );
    }
}

fn running(store: Res<WorldStore>) -> bool {
    store.running.is_some()
}

fn pending(store: Res<WorldStore>) -> bool {
    !store.pending.is_empty()
}

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::alloc_in_tick,
        reason = "runs only on a world list request, and the jobs move into the pending queue"
    )
)]
fn queue_jobs(
    mut store: ResMut<WorldStore>,
    mut list: ResMut<WorldList>,
    mut registries: ResMut<DataPackRegistries>,
    mut types: ResMut<WorldTypes>,
    mut reads: MessageReader<ReadWorldList>,
    mut creates: MessageReader<CreateWorld>,
    mut deletes: MessageReader<DeleteWorld>,
) {
    let reads = reads.read().map(|_| Job::Read);
    let creates = creates.read().map(|create| Job::Create(create.0.clone()));
    let deletes = deletes
        .read()
        .filter(|delete| listed(&list, &delete.folder_id).is_some())
        .map(|delete| Job::Delete(delete.folder_id.clone()));
    let mut jobs: Vec<Job> = reads.chain(creates).chain(deletes).collect();
    let needs_registries = jobs
        .iter()
        .any(|job| matches!(job, Job::Read | Job::Create(_)));
    let unloaded = matches!(
        *registries,
        DataPackRegistries::Unloaded | DataPackRegistries::Failed(_)
    );
    if needs_registries && unloaded {
        let at = jobs
            .iter()
            .take_while(|job| matches!(job, Job::Read))
            .count();
        jobs.insert(at, Job::LoadRegistries);
        *registries = DataPackRegistries::Loading;
        types.set_if_neq(WorldTypes::Loading);
    }
    store.pending.extend(jobs);
    if read_pending(&store) {
        list.set_if_neq(WorldList::Reading);
    }
}

fn read_pending(store: &WorldStore) -> bool {
    store
        .pending
        .iter()
        .any(|job| matches!(job, Job::Read | Job::Delete(_)))
}

fn listed<'a>(list: &'a WorldList, folder_id: &str) -> Option<&'a WorldSummary> {
    let WorldList::Ready(worlds) = list else {
        return None;
    };
    let mut components = Path::new(folder_id).components();
    let single =
        matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none();
    worlds
        .iter()
        .find(|world| single && world.folder_id == folder_id)
}

fn start_job(
    mut store: ResMut<WorldStore>,
    mut list: ResMut<WorldList>,
    registries: Res<DataPackRegistries>,
) {
    let Some(job) = store.pending.pop_front() else {
        return;
    };
    let plugin = store.plugin.clone();
    if matches!(job, Job::Read | Job::Delete(_)) {
        list.set_if_neq(WorldList::Reading);
    }
    let registries = match (&job, &*registries) {
        (Job::Create(_), DataPackRegistries::Ready(registries)) => Ok(registries.clone()),
        (_, DataPackRegistries::Failed(error)) => Err(*error),
        _ => Err(WorldStoreError::DataPack),
    };
    store.running = Some(IoTaskPool::get().spawn(async move { run(job, registries, &plugin) }));
}

fn run(
    job: Job,
    registries: Result<Registries, WorldStoreError>,
    plugin: &WorldListPlugin,
) -> Done {
    let saves = &plugin.saves;
    match job {
        Job::Read => Done::Read(read(saves)),
        Job::LoadRegistries => Done::Registries(load(plugin)),
        Job::Create(world) => {
            Done::Created(registries.and_then(|registries| create(&world, &registries, saves)))
        }
        Job::Open(folder_id) => {
            let world = open(&saves.join(&folder_id));
            Done::Opened { folder_id, world }
        }
        Job::Delete(folder_id) => {
            let removed = delete_world(&saves.join(&folder_id)).map_err(|error| (&error).into());
            Done::Deleted {
                folder_id,
                removed,
                list: read(saves),
            }
        }
    }
}

fn load(plugin: &WorldListPlugin) -> Result<Registries, WorldStoreError> {
    let started = Instant::now();
    let registries = data_pack_registries(&plugin.server)?;
    info!(elapsed = ?started.elapsed(), "loaded the registries of the world types");
    Ok(registries)
}

fn read(saves: &Path) -> Result<Vec<WorldSummary>, WorldStoreError> {
    read_saves(saves).map_err(|error| {
        error!(%error, "the saves folder cannot be listed");
        (&error).into()
    })
}

fn open(folder: &Path) -> Result<LockedWorld, WorldStoreError> {
    let lock = SaveLock::take(folder).map_err(|error| {
        error!(%error, "the world folder is not locked");
        WorldStoreError::from(&error)
    })?;
    let host = restore_level_dat_backup(folder)
        .and_then(|()| read_level_dat(folder))
        .ok()
        .and_then(|level| level.singleplayer_uuid);
    Ok(LockedWorld { lock, host })
}

fn create(
    world: &NewWorld,
    registries: &Registries,
    saves: &Path,
) -> Result<SaveLock, WorldStoreError> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|since| i64::try_from(since.as_millis()).ok())
        .unwrap_or(0);
    create_world(saves, world, registries, now).map_err(|error| {
        error!(%error, "the new world was not written");
        (&error).into()
    })
}

fn finish_job(
    mut store: ResMut<WorldStore>,
    mut list: ResMut<WorldList>,
    mut registries: ResMut<DataPackRegistries>,
    mut types: ResMut<WorldTypes>,
    mut failed: MessageWriter<WorldStoreFailed>,
    mut opens: MessageWriter<OpenLocalWorld>,
) {
    let Some(done) = store.running.as_mut().and_then(check_ready) else {
        return;
    };
    store.running = None;
    let read_pending = read_pending(&store);
    match done {
        Done::Read(read) => {
            if !read_pending {
                list.set_if_neq(listed_worlds(read));
            }
        }
        Done::Registries(Ok(loaded)) => {
            types.set_if_neq(WorldTypes::of(&loaded));
            *registries = DataPackRegistries::Ready(loaded);
        }
        Done::Registries(Err(error)) => {
            types.set_if_neq(WorldTypes::Failed(error));
            *registries = DataPackRegistries::Failed(error);
        }
        Done::Created(Ok(lock)) => {
            opens.write(OpenLocalWorld {
                folder: lock.dir().to_owned(),
                host: None,
                origin: LocalWorldOrigin::Created,
                lock: Some(lock),
            });
        }
        Done::Created(Err(error)) => {
            failed.write(WorldStoreFailed::Create(error));
        }
        Done::Opened {
            world: Ok(LockedWorld { lock, host }),
            ..
        } => {
            opens.write(OpenLocalWorld {
                folder: lock.dir().to_owned(),
                host,
                origin: LocalWorldOrigin::Opened,
                lock: Some(lock),
            });
        }
        Done::Opened {
            folder_id,
            world: Err(error),
        } => {
            failed.write(WorldStoreFailed::Play { folder_id, error });
        }
        Done::Deleted {
            folder_id,
            removed,
            list: read,
        } => {
            if let Err(error) = removed {
                failed.write(WorldStoreFailed::Delete { folder_id, error });
            }
            if !read_pending {
                list.set_if_neq(listed_worlds(read));
            }
        }
    }
}

fn listed_worlds(read: Result<Vec<WorldSummary>, WorldStoreError>) -> WorldList {
    match read {
        Ok(worlds) => WorldList::Ready(worlds),
        Err(error) => WorldList::Unavailable(error),
    }
}

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::alloc_in_tick,
        reason = "runs only on a play request, and the folder id moves into the job queue"
    )
)]
fn play_world(
    mut store: ResMut<WorldStore>,
    list: Res<WorldList>,
    mut plays: MessageReader<PlayWorld>,
    mut failed: MessageWriter<WorldStoreFailed>,
) {
    for play in plays.read() {
        match listed(&list, &play.folder_id) {
            Some(world) if !world.is_disabled() => {
                store.pending.push_back(Job::Open(world.folder_id.clone()));
            }
            refused => {
                let error = match refused {
                    None => WorldStoreError::Io(std::io::ErrorKind::NotFound),
                    Some(world) if world.locked => WorldStoreError::Held,
                    Some(_) => WorldStoreError::Unplayable,
                };
                failed.write(WorldStoreFailed::Play {
                    folder_id: play.folder_id.clone(),
                    error,
                });
            }
        }
    }
}

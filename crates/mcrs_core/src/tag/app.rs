use crate::state::AppState;
use crate::tag::bitset::TagId;
use crate::tag::file::TagFile;
use crate::tag::key::{TagKey, TaggedRegistry};
use crate::tag::registry::{TagLoader, TagSource, resolve_tag_file};
use bevy_app::{App, Update};
use bevy_asset::{AssetServer, Assets};
use bevy_ecs::prelude::*;
use bevy_state::prelude::*;

/// The phases a tagged registry passes through, as system sets so callers can
/// order their own systems against them.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum TagPhase {
    /// `OnEnter(LoadingDataPack)`: ask the asset server for every declared tag file.
    Request,
    Reset,
    /// `Update` while loading: report whether every tag file has settled.
    Settled,
    /// `OnEnter(WorldgenFreeze)`: expand tag files into id sets.
    Resolve,
    /// `OnEnter(WorldgenFreeze)`: consume each loader into its `TagRegistry`.
    Freeze,
}

/// `true` while every registered [`TagLoader`] has all its tag files settled.
#[derive(Resource)]
pub struct TagLoadersSettled(bool);

impl Default for TagLoadersSettled {
    fn default() -> Self {
        TagLoadersSettled(true)
    }
}

impl TagLoadersSettled {
    pub fn get(&self) -> bool {
        self.0
    }
}

pub trait TagRegistryAppExt {
    /// Declare a tagged registry: its tag files are requested while the data
    /// pack loads, resolved against `S` at `WorldgenFreeze`, and the loader is
    /// consumed into a `TagRegistry<T, S::Id>` in [`TagPhase::Freeze`].
    fn add_tagged_registry<T, S>(&mut self, tags: &'static [TagKey<T>]) -> &mut Self
    where
        T: TaggedRegistry + 'static,
        S: TagSource + Resource;
}

impl TagRegistryAppExt for App {
    fn add_tagged_registry<T, S>(&mut self, tags: &'static [TagKey<T>]) -> &mut Self
    where
        T: TaggedRegistry + 'static,
        S: TagSource + Resource,
    {
        if !self.world().contains_resource::<TagLoadersSettled>() {
            self.init_resource::<TagLoadersSettled>()
                .configure_sets(Update, (TagPhase::Reset, TagPhase::Settled).chain())
                .configure_sets(
                    OnEnter(AppState::WorldgenFreeze),
                    (TagPhase::Resolve, TagPhase::Freeze).chain(),
                )
                .add_systems(
                    Update,
                    reset_settled
                        .in_set(TagPhase::Reset)
                        .run_if(in_state(AppState::LoadingDataPack)),
                );
        }

        self.insert_resource(TagLoader::<T, S::Id>::new(tags))
            .add_systems(
                OnEnter(AppState::LoadingDataPack),
                request_tags::<T, S::Id>.in_set(TagPhase::Request),
            )
            .add_systems(
                Update,
                check_settled::<T, S::Id>
                    .in_set(TagPhase::Settled)
                    .run_if(in_state(AppState::LoadingDataPack)),
            )
            .add_systems(
                OnEnter(AppState::WorldgenFreeze),
                (
                    resolve_tags::<T, S>.in_set(TagPhase::Resolve),
                    freeze_tags::<T, S>.in_set(TagPhase::Freeze),
                ),
            )
    }
}

fn reset_settled(mut settled: ResMut<TagLoadersSettled>) {
    settled.0 = true;
}

fn check_settled<T: TaggedRegistry + 'static, I: TagId>(
    loader: Res<TagLoader<T, I>>,
    asset_server: Res<AssetServer>,
    mut settled: ResMut<TagLoadersSettled>,
) {
    if !loader.all_handles_settled(&asset_server) {
        settled.0 = false;
    }
}

fn request_tags<T: TaggedRegistry + 'static, I: TagId>(
    mut loader: ResMut<TagLoader<T, I>>,
    asset_server: Res<AssetServer>,
) {
    let tags = loader.requested();
    for tag in tags {
        loader.request(tag, &asset_server);
    }
    tracing::info!(
        count = tags.len(),
        registry = T::REGISTRY_PATH,
        "requested tag files"
    );
}

fn resolve_tags<T: TaggedRegistry + 'static, S: TagSource + Resource>(
    mut loader: ResMut<TagLoader<T, S::Id>>,
    tag_files: Res<Assets<TagFile>>,
    source: Res<S>,
) {
    let mut resolved = 0usize;
    for (loc, handle) in loader.drain_handles() {
        match tag_files.get(&handle) {
            Some(tag_file) => {
                let ids = resolve_tag_file(tag_file, &tag_files, &*source);
                resolved += ids.len();
                loader.insert(loc, ids);
            }
            None => tracing::warn!("tag file not available at WorldgenFreeze: {loc}"),
        }
    }
    tracing::info!(
        resolved_entries = resolved,
        registry = T::REGISTRY_PATH,
        "resolved tags"
    );
}

fn freeze_tags<T: TaggedRegistry + 'static, S: TagSource + Resource>(world: &mut World) {
    let loader = world
        .remove_resource::<TagLoader<T, S::Id>>()
        .expect("tagged registry frozen twice");
    let registry = {
        let source = world.resource::<S>();
        loader.freeze(source)
    };
    world.insert_resource(registry);
    tracing::info!(registry = T::REGISTRY_PATH, "frozen tag registry");
}

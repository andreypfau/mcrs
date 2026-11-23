use std::path::Path;
use std::sync::Arc;

use bevy::app::{App, Plugin, Update};
use bevy::asset::AssetServer;
use bevy::asset::io::AssetSourceId;
use bevy::ecs::schedule::SystemSet;
use bevy::log::{error, info};
use bevy::prelude::*;
use bevy::tasks::futures::check_ready;
use bevy::tasks::{AsyncComputeTaskPool, Task};
use mcrs_minecraft_block::definition::{
    BlockDefinitions, Blocks, LoadError, LoadReport, load_block_definitions,
};
use mcrs_minecraft_datapack::DirectoryPacks;
use mcrs_minecraft_engine_item::ItemPlugin;
use mcrs_minecraft_engine_registries::{
    ItemTableInputs, LoadedItems, load_item_definitions, load_items,
};
use mcrs_minecraft_environment::Timeline;
use mcrs_minecraft_environment::{ClockTimeMarkers, WorldClock, WorldClockPlugin, WorldClocks};
use mcrs_minecraft_game_registries::registry_load::{RegistryLoadError, load_received};
use mcrs_minecraft_inventory_client::InventoryClientPlugin;
use mcrs_minecraft_item::Items;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_network::ConnectionState;
use mcrs_minecraft_network::client::{
    ClientNetworkSystems, ConnectionEnd, ConnectionEnded, RegistriesReceived,
    close_connection_later, finish_configuration,
};
use mcrs_minecraft_network::event;
use mcrs_minecraft_player_holders::PlayerHoldersPlugin;
use mcrs_minecraft_protocol_game::clientbound::ClientboundUpdateTags;
use mcrs_minecraft_registries::{CurrentRegistries, Diagnostics, Received, Registries};

use crate::collision_tables::{ClientPhysicsTables, build_collision_tables};
use crate::item_table_cache::{BakedItemTable, ItemTableCache};
use crate::joined_game::{self, CurrentDimension};
use crate::received::{received, received_tags};
use crate::session::session_boundary;

const DATAPACK_REPORT: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/mcrs/reports/datapack.json"
));

#[derive(Resource)]
pub struct LocalCorpus(DirectoryPacks);

#[derive(Component)]
pub struct LastConfiguration {
    pub received: Received,
    pub vanilla_known: bool,
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SessionSet {
    Clear,
    Derive,
}

#[derive(Default)]
pub struct ClientRegistriesPlugin {
    pub local_corpus: Option<DirectoryPacks>,
    pub defer_definitions: bool,
}

#[derive(Resource)]
struct PendingBlocks(Task<Result<(BlockDefinitions, LoadReport), LoadError>>);

impl Plugin for ClientRegistriesPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<ItemPlugin>() {
            app.add_plugins(ItemPlugin::default());
        }
        if !app.is_plugin_added::<PlayerHoldersPlugin>() {
            app.add_plugins(PlayerHoldersPlugin);
        }
        if !app.is_plugin_added::<InventoryClientPlugin>() {
            app.add_plugins(InventoryClientPlugin);
        }
        app.add_plugins(WorldClockPlugin)
            .init_resource::<ItemTableCache>()
            .add_message::<ConnectionEnded>()
            .configure_sets(
                Update,
                (
                    SessionSet::Clear.after(ClientNetworkSystems::Receive),
                    SessionSet::Derive.after(SessionSet::Clear),
                ),
            )
            .add_systems(
                Update,
                (
                    clear_derived_tables
                        .in_set(SessionSet::Clear)
                        .run_if(session_boundary),
                    (
                        derive_clock_markers,
                        seed_session_clocks,
                        build_collision_tables,
                    )
                        .in_set(SessionSet::Derive)
                        .run_if(resource_exists_and_changed::<CurrentRegistries>),
                ),
            )
            .add_observer(load_server_registries)
            .add_observer(replace_server_tags)
            .add_systems(
                First,
                insert_pending_blocks.run_if(resource_exists::<PendingBlocks>),
            )
            .configure_sets(
                Update,
                ClientNetworkSystems::Deliver.run_if(resource_exists::<Blocks>),
            );
        joined_game::add_observers(app);
        crate::session::add_observers(app);
        crate::clock_sync::add_systems(app);
        crate::pong::add_observers(app);
    }

    fn finish(&self, app: &mut App) {
        let assets = app.world().resource::<AssetServer>().clone();
        check_corpus_version(&assets).unwrap_or_else(|error| refuse(&error));
        if self.defer_definitions {
            app.insert_resource(PendingBlocks(
                AsyncComputeTaskPool::get().spawn(async move { load_block_definitions(&assets) }),
            ));
        } else {
            let loaded = load_block_definitions(&assets);
            app.insert_resource(adopt(loaded));
        }
        if let Some(corpus) = &self.local_corpus {
            app.insert_resource(LocalCorpus(corpus.clone()));
        }
    }
}

fn adopt(loaded: Result<(BlockDefinitions, LoadReport), LoadError>) -> Blocks {
    let (definitions, report) = loaded.unwrap_or_else(|error| refuse(&error));
    info!(
        blocks = definitions.blocks().len(),
        states = report.states,
        permutations = report.permutations,
        shapes = report.shapes,
        bytes = report.table_bytes,
        elapsed = ?report.elapsed,
        "loaded block definitions"
    );
    Blocks(Arc::new(definitions))
}

fn insert_pending_blocks(mut pending: ResMut<PendingBlocks>, mut commands: Commands) {
    if let Some(loaded) = check_ready(&mut pending.0) {
        commands.remove_resource::<PendingBlocks>();
        commands.insert_resource(adopt(loaded));
    }
}

fn refuse<E: std::fmt::Display>(error: &E) -> ! {
    error!("the registries are unusable:\n{error}");
    std::process::exit(1)
}

pub fn load_registries(
    configuration: &LastConfiguration,
    corpus: Option<&LocalCorpus>,
) -> Result<(Registries, Diagnostics), RegistryLoadError> {
    let vanilla = corpus.filter(|_| configuration.vanilla_known);
    load_received(
        DATAPACK_REPORT,
        &configuration.received,
        vanilla.map(|corpus| &corpus.0 as _),
    )
}

fn install(
    (registries, _): (Registries, Diagnostics),
    (assets, blocks): (&AssetServer, &Blocks),
    (cache, current): (&ItemTableCache, Option<&ItemTable>),
    mut commands: Commands<'_, '_>,
) {
    let inputs = ItemTableInputs::of(&registries);
    let reused = cache
        .0
        .as_ref()
        .filter(|baked| baked.inputs == inputs)
        .map(|baked| baked.table);
    let loaded = match reused {
        Some(table) => load_item_definitions(assets, &registries, blocks)
            .map(|definitions| LoadedItems { definitions, table }),
        None => load_items(assets, &registries, blocks),
    };
    match loaded {
        Ok(LoadedItems { definitions, table }) => {
            info!(
                items = definitions.len(),
                rebaked = reused.is_none(),
                "derived the item table from the session registries"
            );
            commands.insert_resource(Items(Arc::new(definitions)));
            if reused.is_none() || current.is_none() {
                commands.insert_resource(table);
            }
            if reused.is_none() {
                commands.insert_resource(ItemTableCache(Some(BakedItemTable { inputs, table })));
            }
        }
        Err(error) => {
            error!(%error, "the item definitions do not fit the session registries");
            commands.remove_resource::<CurrentRegistries>();
            commands.remove_resource::<Items>();
            commands.remove_resource::<ItemTable>();
        }
    }
    commands.insert_resource(CurrentRegistries::new(registries));
}

fn load_server_registries(
    event: On<RegistriesReceived>,
    corpus: Option<Res<LocalCorpus>>,
    assets: Res<AssetServer>,
    blocks: Res<Blocks>,
    (cache, current): (Res<ItemTableCache>, Option<Res<ItemTable>>),
    mut commands: Commands,
) {
    let configuration = match received(&event.configuration) {
        Ok(received) => LastConfiguration {
            received,
            vanilla_known: event.configuration.vanilla_known,
        },
        Err(error) => {
            close_connection_later(
                commands.reborrow(),
                event.entity,
                ConnectionEnd::Lost(format!("the server's registries cannot be read: {error:#}")),
            );
            return;
        }
    };
    match load_registries(&configuration, corpus.as_deref()) {
        Ok(loaded) => {
            install(
                loaded,
                (&assets, &blocks),
                (&cache, current.as_deref()),
                commands.reborrow(),
            );
            commands.entity(event.entity).insert(configuration);
            finish_configuration(commands.reborrow(), event.entity);
        }
        Err(error) => close_connection_later(
            commands.reborrow(),
            event.entity,
            ConnectionEnd::Lost(format!("the server's registries cannot be used:\n{error}")),
        ),
    }
}

fn replace_server_tags(
    event: On<event::Received<ClientboundUpdateTags<'static>>>,
    mut connections: Query<&mut LastConfiguration>,
    corpus: Option<Res<LocalCorpus>>,
    assets: Res<AssetServer>,
    blocks: Res<Blocks>,
    (cache, current): (Res<ItemTableCache>, Option<Res<ItemTable>>),
    mut commands: Commands,
) {
    let Some(update) = event.packet() else {
        return;
    };
    let Ok(mut last) = connections.get_mut(event.entity) else {
        close_connection_later(
            commands.reborrow(),
            event.entity,
            ConnectionEnd::Lost("the server sent tags before its registries".to_owned()),
        );
        return;
    };
    last.received.replace_tags(received_tags(update.registries));
    match load_registries(&last, corpus.as_deref()) {
        Ok(loaded) => install(
            loaded,
            (&assets, &blocks),
            (&cache, current.as_deref()),
            commands.reborrow(),
        ),
        Err(error) => close_connection_later(
            commands.reborrow(),
            event.entity,
            ConnectionEnd::Lost(format!("the server's tags cannot be used:\n{error}")),
        ),
    }
}

fn check_corpus_version(assets: &AssetServer) -> Result<(), String> {
    let source = assets
        .get_source(AssetSourceId::Default)
        .map_err(|error| error.to_string())?;
    let path = Path::new("minecraft/version.json");
    let bytes = bevy::tasks::block_on(mcrs_minecraft_assets::asset::read_whole(
        source.reader(),
        path,
    ))
    .map_err(|error| format!("{}: {error}", path.display()))?;
    mcrs_minecraft_core::check_corpus_version(&bytes)
        .map_err(|error| format!("{}: {error}", path.display()))
}

pub fn entered_configuration(
    connections: Query<&ConnectionState, Changed<ConnectionState>>,
) -> bool {
    connections
        .iter()
        .any(|state| *state == ConnectionState::Configuration)
}

fn clear_derived_tables(
    mut clocks: ResMut<WorldClocks>,
    dimensions: Query<Entity, With<CurrentDimension>>,
    mut commands: Commands,
) {
    commands.remove_resource::<CurrentRegistries>();
    commands.remove_resource::<Items>();
    commands.remove_resource::<ItemTable>();
    commands.remove_resource::<ClockTimeMarkers>();
    commands.remove_resource::<ClientPhysicsTables>();
    clocks.set_if_neq(WorldClocks::default());
    for connection in &dimensions {
        commands.entity(connection).remove::<CurrentDimension>();
    }
}

fn derive_clock_markers(registries: Res<CurrentRegistries>, mut commands: Commands) {
    let Some(timelines) = registries.registry::<Timeline>() else {
        error!("the session registries hold no timelines to read time markers from");
        commands.remove_resource::<ClockTimeMarkers>();
        return;
    };
    commands.insert_resource(ClockTimeMarkers::new(timelines.values()));
}

fn seed_session_clocks(mut clocks: ResMut<WorldClocks>, registries: Res<CurrentRegistries>) {
    clocks.reconcile(
        registries
            .registry::<WorldClock>()
            .map_or(0, |registry| registry.len()),
    );
}

#[cfg(test)]
mod tests {
    use std::sync::LazyLock;

    use bevy::asset::AssetPlugin;
    use bevy::ecs::change_detection::Tick;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::prelude::*;
    use mcrs_minecraft_block::definition::Blocks;
    use mcrs_minecraft_game_registries::registry_load::{load_received, sent_by, test_registries};
    use mcrs_minecraft_item_stack::Kind;
    use mcrs_minecraft_item_table::{ItemTable, Prototype};
    use mcrs_minecraft_registries::{Diagnostics, Registries};

    use super::{ClientRegistriesPlugin, DATAPACK_REPORT, install};
    use crate::item_table_cache::ItemTableCache;

    static WITHOUT_ONE_BLOCK_TAG: LazyLock<Registries> = LazyLock::new(|| {
        let mut received = sent_by(test_registries(), false);
        let (_, block_tags) = received
            .tags
            .iter_mut()
            .find(|(registry, _)| *registry == "minecraft:block")
            .unwrap();
        block_tags.pop().unwrap();
        load_received(DATAPACK_REPORT, &received, None).unwrap().0
    });

    #[derive(Debug, PartialEq, Eq)]
    enum Table {
        Reused,
        Reinserted,
        Baked,
    }

    struct Installed {
        prototype: *const Prototype,
        changed: Tick,
    }

    fn install_session(app: &mut App, registries: &Registries) -> Installed {
        let registries = registries.clone();
        app.world_mut()
            .run_system_once(
                move |assets: Res<AssetServer>,
                      blocks: Res<Blocks>,
                      cache: Res<ItemTableCache>,
                      current: Option<Res<ItemTable>>,
                      mut commands: Commands| {
                    install(
                        (registries.clone(), Diagnostics::default()),
                        (&assets, &blocks),
                        (&cache, current.as_deref()),
                        commands.reborrow(),
                    );
                },
            )
            .unwrap();
        app.world_mut().increment_change_tick();
        let table = app.world().resource_ref::<ItemTable>();
        Installed {
            prototype: table.prototype(Kind(1)).unwrap(),
            changed: table.last_changed(),
        }
    }

    struct Case {
        name: &'static str,
        second: &'static Registries,
        expected: Table,
    }

    #[test]
    fn a_second_install_reuses_the_baked_table_only_for_equal_inputs() {
        let cases = [
            Case {
                name: "the same registries",
                second: test_registries(),
                expected: Table::Reused,
            },
            Case {
                name: "one block tag less",
                second: &WITHOUT_ONE_BLOCK_TAG,
                expected: Table::Baked,
            },
        ];
        for case in cases {
            let mut app = App::new();
            app.add_plugins((
                TaskPoolPlugin::default(),
                AssetPlugin {
                    watch_for_changes_override: Some(false),
                    ..Default::default()
                },
                ClientRegistriesPlugin::default(),
            ));
            app.finish();
            app.cleanup();

            let first = install_session(&mut app, test_registries());
            let second = install_session(&mut app, case.second);

            let table = if first.prototype != second.prototype {
                Table::Baked
            } else if first.changed != second.changed {
                Table::Reinserted
            } else {
                Table::Reused
            };
            assert_eq!(table, case.expected, "{}", case.name);
        }
    }

    #[test]
    fn deferred_block_definitions_arrive_after_finish_and_not_before() {
        let mut app = App::new();
        app.add_plugins((
            TaskPoolPlugin::default(),
            AssetPlugin {
                watch_for_changes_override: Some(false),
                ..Default::default()
            },
            ClientRegistriesPlugin {
                defer_definitions: true,
                ..default()
            },
        ));
        app.finish();
        app.cleanup();
        assert!(app.world().get_resource::<Blocks>().is_none());

        let started = std::time::Instant::now();
        while app.world().get_resource::<Blocks>().is_none() {
            assert!(
                started.elapsed() < std::time::Duration::from_secs(60),
                "the block definitions never arrived"
            );
            app.update();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(app.world().resource::<Blocks>().state_count() > 0);
    }
}

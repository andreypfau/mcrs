use std::path::Path;
use std::sync::Arc;

use bevy::app::{App, Plugin, Update};
use bevy::asset::AssetServer;
use bevy::asset::io::AssetSourceId;
use bevy::ecs::schedule::SystemSet;
use bevy::log::{error, info};
use bevy::prelude::*;
use mcrs_minecraft_block::definition::{Blocks, load_block_definitions};
use mcrs_minecraft_core::Id;
use mcrs_minecraft_environment::timeline::Timeline;
use mcrs_minecraft_environment::world_clock::{
    ClockTimeMarkers, WorldClock, WorldClockPlugin, WorldClocks,
};
use mcrs_minecraft_item::Items;
use mcrs_minecraft_network::ConnectionState;
use mcrs_minecraft_network::client::{
    ClientNetworkSystems, CurrentDimension, HoldsVanillaPack, RegistriesReceived, ServerRegistries,
    TagsReceived, close_connection_later, finish_configuration,
};
use mcrs_minecraft_registries::{CurrentRegistries, RegistryOf};
use mcrs_minecraft_world::item::definitions::load_item_definitions;
use mcrs_minecraft_world::registries::refuse;
use mcrs_minecraft_world::registry_load::{
    DirectoryPacks, KnownEntries, LoadedRegistries, PackSource, RegistryLoadError, load_received,
    load_vanilla_registries,
};

use crate::blocks::BiomeTints;

const DATAPACK_REPORT: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/mcrs/reports/datapack.json"
));

/// The network form of the client's own vanilla pack, which fills the entries a server leaves out.
#[derive(Resource)]
pub(crate) struct VanillaEntries(KnownEntries);

/// What the server sent in its last configuration. A tag update in play replaces the tags of the
/// registries it names and loads the registries again from it.
#[derive(Component)]
struct LastConfiguration(ServerRegistries);

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SessionSet {
    Clear,
    Derive,
}

#[derive(Default)]
pub struct ClientRegistriesPlugin {
    /// The packs the vanilla pack's entries are read from, to fill what a server leaves out.
    /// A client without them answers the server's pack offer with none.
    pub local_corpus: Option<DirectoryPacks>,
}

impl Plugin for ClientRegistriesPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(WorldClockPlugin)
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
                        .run_if(entered_configuration),
                    (derive_clock_markers, seed_session_clocks)
                        .in_set(SessionSet::Derive)
                        .run_if(resource_exists_and_changed::<CurrentRegistries>),
                ),
            )
            .add_observer(load_server_registries)
            .add_observer(replace_server_tags);
    }

    fn finish(&self, app: &mut App) {
        let assets = app.world().resource::<AssetServer>().clone();
        check_corpus_version(&assets);
        let (definitions, report) =
            load_block_definitions(&assets).unwrap_or_else(|error| refuse(&error));
        info!(
            blocks = definitions.blocks().len(),
            states = report.states,
            permutations = report.permutations,
            shapes = report.shapes,
            bytes = report.table_bytes,
            elapsed = ?report.elapsed,
            "loaded block definitions"
        );
        app.insert_resource(Blocks(Arc::new(definitions)));
        if let Some(corpus) = &self.local_corpus {
            app.insert_resource(VanillaEntries(vanilla_entries(corpus)))
                .insert_resource(HoldsVanillaPack);
        }
    }
}

fn vanilla_entries(source: &dyn PackSource) -> KnownEntries {
    load_vanilla_registries(source)
        .and_then(|vanilla| KnownEntries::of(&vanilla.registries))
        .unwrap_or_else(|error| refuse(&error))
}

pub(crate) fn load_registries(
    received: &ServerRegistries,
    vanilla: Option<&VanillaEntries>,
) -> Result<LoadedRegistries, RegistryLoadError> {
    let known = vanilla
        .filter(|_| received.vanilla_known)
        .map(|vanilla| &vanilla.0);
    load_received(
        DATAPACK_REPORT,
        known,
        (received.registries.clone(), received.tags.clone()),
    )
}

/// The item table is derived from the loaded registries as they are installed, so no frame sees
/// the numbers of one load beside the values of another.
fn install(
    loaded: LoadedRegistries,
    (assets, blocks): (&AssetServer, &Blocks),
    commands: &mut Commands,
) {
    match load_item_definitions(assets, &loaded.registries, blocks) {
        Ok(items) => {
            info!(
                items = items.len(),
                "derived the item table from the session registries"
            );
            commands.insert_resource(Items(Arc::new(items)));
        }
        Err(error) => {
            error!(%error, "the item definitions do not fit the session registries");
            commands.remove_resource::<Items>();
        }
    }
    commands.insert_resource(CurrentRegistries::new(loaded.registries));
}

fn load_server_registries(
    event: On<RegistriesReceived>,
    vanilla: Option<Res<VanillaEntries>>,
    assets: Res<AssetServer>,
    blocks: Res<Blocks>,
    mut commands: Commands,
) {
    match load_registries(&event.registries, vanilla.as_deref()) {
        Ok(loaded) => {
            install(loaded, (&assets, &blocks), &mut commands);
            commands
                .entity(event.entity)
                .insert(LastConfiguration(event.registries.clone()));
            finish_configuration(&mut commands, event.entity);
        }
        Err(error) => close_connection_later(
            &mut commands,
            event.entity,
            format!("the server's registries cannot be used:\n{error}"),
        ),
    }
}

fn replace_server_tags(
    event: On<TagsReceived>,
    mut connections: Query<&mut LastConfiguration>,
    vanilla: Option<Res<VanillaEntries>>,
    assets: Res<AssetServer>,
    blocks: Res<Blocks>,
    mut commands: Commands,
) {
    let Ok(mut last) = connections.get_mut(event.entity) else {
        close_connection_later(
            &mut commands,
            event.entity,
            "the server sent tags before its registries".to_owned(),
        );
        return;
    };
    last.0.replace_tags(event.tags.clone());
    match load_registries(&last.0, vanilla.as_deref()) {
        Ok(loaded) => install(loaded, (&assets, &blocks), &mut commands),
        Err(error) => close_connection_later(
            &mut commands,
            event.entity,
            format!("the server's tags cannot be used:\n{error}"),
        ),
    }
}

fn check_corpus_version(assets: &AssetServer) {
    let source = assets
        .get_source(AssetSourceId::Default)
        .expect("default AssetSource missing");
    let path = Path::new("minecraft/version.json");
    let bytes = bevy::tasks::block_on(mcrs_minecraft_assets::asset::read_whole(
        source.reader(),
        path,
    ))
    .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    mcrs_minecraft_core::check_corpus_version(&bytes)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
}

/// True once a connection has entered configuration, from play or at its start. The server
/// sends the next registries only after the client's acknowledgement, a frame or more later, so
/// no set arrives before the clearing this gates has run.
pub(crate) fn entered_configuration(
    connections: Query<&ConnectionState, Changed<ConnectionState>>,
) -> bool {
    connections
        .iter()
        .any(|state| *state == ConnectionState::Configuration)
}

/// The tables resolved from the previous set hold its numbers. They are dropped here and built
/// again from the set that replaces it, so no table holds the numbers of two sets.
fn clear_derived_tables(
    mut clocks: ResMut<WorldClocks>,
    dimensions: Query<Entity, With<CurrentDimension>>,
    mut commands: Commands,
) {
    commands.remove_resource::<Items>();
    commands.remove_resource::<ClockTimeMarkers>();
    commands.remove_resource::<BiomeTints>();
    if !clocks.is_empty() {
        *clocks = WorldClocks::default();
    }
    for connection in &dimensions {
        commands.entity(connection).remove::<CurrentDimension>();
    }
}

fn derive_clock_markers(registries: Res<CurrentRegistries>, mut commands: Commands) {
    let (Some(clocks), Some(timelines)) = (
        registries.registry::<RegistryOf<WorldClock>>(),
        registries.registry::<RegistryOf<Timeline>>(),
    ) else {
        error!(
            "the session registries hold no world clocks and timelines to read time markers from"
        );
        commands.remove_resource::<ClockTimeMarkers>();
        return;
    };
    let clock_name =
        |clock: Id<WorldClock>| clocks.name(Id::checked(clock.number(), clocks.len())?);
    match ClockTimeMarkers::derive(timelines.values(), clock_name) {
        Ok(markers) => commands.insert_resource(markers),
        Err(duplicates) => {
            for (timeline, duplicate) in duplicates {
                error!(timeline, %duplicate, "the session defines a time marker twice");
            }
            commands.remove_resource::<ClockTimeMarkers>();
        }
    }
}

fn seed_session_clocks(mut clocks: ResMut<WorldClocks>, registries: Res<CurrentRegistries>) {
    clocks.reconcile(
        registries
            .registry::<RegistryOf<WorldClock>>()
            .map_or(0, |registry| registry.len()),
    );
}

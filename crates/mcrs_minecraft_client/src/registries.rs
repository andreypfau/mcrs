use std::path::{Path, PathBuf};
use std::sync::Arc;

use bevy::app::{App, Plugin, Update};
use bevy::asset::AssetServer;
use bevy::asset::io::AssetSourceId;
use bevy::ecs::schedule::SystemSet;
use bevy::log::{error, info};
use bevy::prelude::*;
use mcrs_minecraft_block::definition::{Blocks, load_block_definitions};
use mcrs_minecraft_block::keys::Block;
use mcrs_minecraft_environment::timeline::Timeline;
use mcrs_minecraft_environment::world_clock::{
    ClockTimeMarkers, WorldClock, WorldClockPlugin, WorldClocks, seed_world_clocks,
};
use mcrs_minecraft_item::Items;
use mcrs_minecraft_network::ConnectionState;
use mcrs_minecraft_network::client::{
    ClientNetworkSystems, CurrentDimension, SessionRegistryInputs,
};
use mcrs_minecraft_registry::RegistrySet;
use mcrs_minecraft_world::item::definitions::load_item_definitions;
use mcrs_minecraft_world::packs::known_pack_entries;
use mcrs_minecraft_world::registries::{refuse, static_registries, world_registries};

use crate::blocks::BiomeTints;

const DATAPACK_REPORT: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/mcrs/reports/datapack.json"
));

pub fn session_inputs(local_corpus: Option<&Path>) -> SessionRegistryInputs {
    SessionRegistryInputs {
        declarations: world_registries(DATAPACK_REPORT).unwrap_or_else(|report| refuse(&report)),
        statics: static_registries().unwrap_or_else(|report| refuse(&report)),
        known: local_corpus
            .map(|root| known_pack_entries(root).unwrap_or_else(|report| refuse(&report))),
    }
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SessionSet {
    Clear,
    Derive,
}

pub struct ClientRegistriesPlugin {
    /// The folder the vanilla pack's entries are read from, to fill what a server leaves out.
    /// A client without one answers the server's pack offer with none.
    pub local_corpus: Option<PathBuf>,
}

impl Default for ClientRegistriesPlugin {
    fn default() -> Self {
        Self {
            #[cfg(not(target_family = "wasm"))]
            local_corpus: Some(crate::asset_corpus()),
            #[cfg(target_family = "wasm")]
            local_corpus: None,
        }
    }
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
                    (derive_items, derive_clock_markers, seed_world_clocks)
                        .in_set(SessionSet::Derive)
                        .run_if(resource_exists_and_changed::<RegistrySet>),
                ),
            );
    }

    fn finish(&self, app: &mut App) {
        let assets = app.world().resource::<AssetServer>().clone();
        check_corpus_version(&assets);
        let inputs = session_inputs(self.local_corpus.as_deref());
        let blocks = inputs
            .statics
            .registry::<Block>()
            .expect("the static registries hold minecraft:block");
        let (definitions, report) =
            load_block_definitions(&assets, &blocks).expect("the block definition corpus loads");
        info!(
            blocks = definitions.blocks().len(),
            states = report.states,
            permutations = report.permutations,
            shapes = report.shapes,
            bytes = report.table_bytes,
            elapsed = ?report.elapsed,
            "loaded block definitions"
        );
        app.insert_resource(Blocks(Arc::new(definitions)))
            .insert_resource(inputs);
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

fn derive_items(
    registries: Res<RegistrySet>,
    blocks: Res<Blocks>,
    assets: Res<AssetServer>,
    mut commands: Commands,
) {
    match load_item_definitions(&assets, &registries, &blocks) {
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
}

fn derive_clock_markers(registries: Res<RegistrySet>, mut commands: Commands) {
    let (Some(clocks), Some(timelines)) = (
        registries.registry::<WorldClock>(),
        registries.column::<Timeline>(
            mcrs_minecraft_environment::keys::TIMELINE
                .location()
                .as_static_str(),
        ),
    ) else {
        error!(
            "the session registries hold no world clocks and timelines to read time markers from"
        );
        commands.remove_resource::<ClockTimeMarkers>();
        return;
    };
    match ClockTimeMarkers::derive(timelines, &clocks) {
        Ok(markers) => commands.insert_resource(markers),
        Err(duplicates) => {
            for (timeline, duplicate) in duplicates {
                error!(timeline, %duplicate, "the session defines a time marker twice");
            }
            commands.remove_resource::<ClockTimeMarkers>();
        }
    }
}

//! The per-dim plugin that drains the despawns of the players and publishes their tile events.

use bevy_app::{App, FixedPostUpdate, FixedPreUpdate, Plugin};
use bevy_ecs::prelude::IntoScheduleConfigs;
use bevy_ecs::schedule::SystemSet;
use mcrs_minecraft_engine_world::voxel_update::VoxelUpdateSet;

use crate::publish_player_tiles::{publish_player_despawn, publish_player_tiles};
use crate::tile_events::TileEvents;

/// `SystemSet` covering the system that publishes the tile events of the players. Living on its
/// own set keeps it from inadvertently parallelising against simulation systems that touch the
/// same Components.
#[derive(SystemSet, Clone, Default, Hash, PartialEq, Eq, Debug)]
pub struct PlayerTrackerSet;

/// Per-dim plugin: registers the despawn drain in `FixedPreUpdate` and the tile events in
/// `FixedPostUpdate`.
pub struct PlayerTrackerPlugin;

impl Plugin for PlayerTrackerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<mcrs_minecraft_engine_world::session::DimPlayerIndex>();
        app.init_resource::<TileEvents>();
        app.add_observer(publish_player_despawn);
        app.configure_sets(
            FixedPostUpdate,
            PlayerTrackerSet.after(VoxelUpdateSet::NetworkSync),
        );
        app.add_systems(
            FixedPreUpdate,
            crate::drain_player_despawn::drain_inbound_player_despawn
                .after(crate::dim_tick::DimInboxDrain),
        );
        app.add_systems(
            FixedPostUpdate,
            publish_player_tiles.in_set(PlayerTrackerSet),
        );
    }
}

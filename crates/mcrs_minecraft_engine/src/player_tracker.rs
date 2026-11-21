use bevy_app::{App, FixedPostUpdate, FixedPreUpdate, Plugin};
use bevy_ecs::prelude::IntoScheduleConfigs;
use mcrs_minecraft_engine_world::voxel_update::VoxelUpdateSet;

use crate::publish_player_tiles::{publish_player_despawn, publish_player_tiles};
use crate::swing::{PendingPlayerSwings, has_pending_player_swings, publish_player_swings};
use crate::tile_events::TileEvents;

pub struct PlayerTrackerPlugin;

impl Plugin for PlayerTrackerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<mcrs_minecraft_engine_world::session::DimPlayerIndex>();
        app.init_resource::<TileEvents>();
        app.init_resource::<PendingPlayerSwings>();
        app.add_observer(publish_player_despawn);
        app.add_systems(
            FixedPreUpdate,
            crate::drain_player_despawn::drain_inbound_player_despawn
                .after(crate::dim_tick::DimInboxDrain),
        );
        app.add_systems(
            FixedPostUpdate,
            (
                publish_player_tiles.after(VoxelUpdateSet::NetworkSync),
                publish_player_swings
                    .run_if(has_pending_player_swings)
                    .after(publish_player_tiles),
            ),
        );
    }
}

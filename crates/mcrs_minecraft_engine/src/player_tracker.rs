//! The per-dim plugin that registers the entity tracker into `FixedPostUpdate`. The plugin also
//! keeps each column's `PlayerObservers` in line with the columns players hold.

use bevy_app::{App, FixedPostUpdate, FixedPreUpdate, Plugin};
use bevy_ecs::prelude::IntoScheduleConfigs;
use bevy_ecs::schedule::SystemSet;
use mcrs_minecraft_engine_world::voxel_update::VoxelUpdateSet;

use crate::bus::PlayerLatency;
use crate::column_held::{
    ColumnHeld, announce_held_columns, mirror_held_columns, withdraw_held_columns,
};
use crate::entity_tracker::EntityTracker;
use crate::join_message::announce_joins;
use crate::latency::{Latency, apply_latency};
use crate::player_dimension_change::CarryOnDimensionChangeExt;
use crate::player_list::{ListPolicy, PlayerList, update_player_list};
use crate::track_entities::{forget_removed_player, track_entities};

/// `SystemSet` covering the AoI system. Living on its own set keeps the AoI
/// work from inadvertently parallelising against simulation systems that
/// touch the same Components.
#[derive(SystemSet, Clone, Default, Hash, PartialEq, Eq, Debug)]
pub struct PlayerTrackerSet;

/// Per-dim plugin: registers the despawn drain in `FixedPreUpdate`, and the observer mirror and the
/// entity tracker in `FixedPostUpdate`.
pub struct PlayerTrackerPlugin;

impl Plugin for PlayerTrackerPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<ColumnHeld>();
        app.init_resource::<mcrs_minecraft_engine_world::session::DimPlayerIndex>();
        app.add_observer(announce_held_columns);
        app.add_observer(withdraw_held_columns);
        app.init_resource::<EntityTracker>();
        app.add_message::<PlayerLatency>();
        app.init_resource::<PlayerList>();
        app.init_resource::<ListPolicy>();
        app.add_observer(forget_removed_player);
        app.carry_on_dimension_change::<Latency>();
        app.configure_sets(
            FixedPostUpdate,
            PlayerTrackerSet.after(VoxelUpdateSet::NetworkSync),
        );
        app.add_systems(
            FixedPreUpdate,
            (
                crate::drain_player_despawn::drain_inbound_player_despawn,
                apply_latency,
            )
                .after(crate::sub_app_builder::DimInboxDrain),
        );
        app.add_systems(
            FixedPostUpdate,
            mirror_held_columns
                .before(PlayerTrackerSet)
                .before(VoxelUpdateSet::NetworkSync),
        );
        app.add_systems(
            FixedPostUpdate,
            (track_entities, update_player_list, announce_joins)
                .chain()
                .in_set(PlayerTrackerSet),
        );
    }
}

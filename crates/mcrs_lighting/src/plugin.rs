use crate::enqueue::{
    enqueue_block_light_on_block_placed, enqueue_sky_light_initial,
    enqueue_sky_light_on_block_placed,
};
use crate::heightmap_update::update_heightmaps_on_block_placed;
use crate::lifecycle::{attach_lighting_state, prime_heightmaps_on_column_spawn};
use crate::propagate::{
    propagate_decrease_block_system, propagate_decrease_sky_system,
    propagate_increase_block_system, propagate_increase_sky_system,
};
use crate::sets::LightingSet;
use crate::table::build_block_light_table;
use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::prelude::{ApplyDeferred, IntoScheduleConfigs};
use bevy_state::prelude::OnEnter;
use mcrs_core::AppState;
use mcrs_engine::world::column::ChunkColumnLifecycleSet;
use mcrs_minecraft::world::block_update::{apply_set_block_request, BlockPlaced, BlockUpdateSet};
use mcrs_vanilla::{freeze_static_tags, transition_to_playing};

pub struct LightingPlugin;

impl Plugin for LightingPlugin {
    fn build(&self, app: &mut App) {
        // `update_heightmaps_on_block_placed` reads `MessageReader<BlockPlaced>`.
        // The production binary also registers `BlockUpdatePlugin`, which calls
        // `add_message::<BlockPlaced>()`. Registering twice would re-initialize
        // the message buffer and drop pending messages, so guard against the
        // duplicate so the lighting plugin stays self-contained for integration
        // tests but no-ops when `BlockUpdatePlugin` has already initialized the
        // buffer.
        if !app
            .world()
            .contains_resource::<bevy_ecs::message::Messages<BlockPlaced>>()
        {
            app.add_message::<BlockPlaced>();
        }

        app.add_systems(
            OnEnter(AppState::WorldgenFreeze),
            build_block_light_table
                .after(freeze_static_tags)
                .before(transition_to_playing),
        );

        // Cross-plugin barrier: the chain begins with a leading `ApplyDeferred`
        // so the spawn `Commands` queued upstream by `reconcile_section_index`
        // are flushed before the heightmap-prime query reads `ChunkColumn` and
        // `SectionIndex` state. The upstream `ColumnPlugin` intentionally
        // omits a trailing post-reconcile `ApplyDeferred`; the responsibility
        // lives here on the consumer side.
        app.add_systems(
            FixedUpdate,
            (
                ApplyDeferred,
                prime_heightmaps_on_column_spawn
                    .in_set(ChunkColumnLifecycleSet::PrimeHeightmaps),
                ApplyDeferred,
                attach_lighting_state.in_set(ChunkColumnLifecycleSet::AttachState),
            )
                .chain()
                .after(ChunkColumnLifecycleSet::ReconcileIndex),
        );

        app.add_systems(
            FixedUpdate,
            update_heightmaps_on_block_placed.after(apply_set_block_request),
        );

        app.configure_sets(
            FixedUpdate,
            (
                LightingSet::Enqueue,
                LightingSet::PropagateDecrease,
                LightingSet::PropagateIncrease,
            )
                .chain()
                .after(BlockUpdateSet::ApplyChanges),
        );

        app.configure_sets(
            FixedUpdate,
            LightingSet::Enqueue.after(ChunkColumnLifecycleSet::AttachState),
        );

        app.add_systems(
            FixedUpdate,
            (
                (
                    enqueue_block_light_on_block_placed,
                    enqueue_sky_light_on_block_placed,
                    enqueue_sky_light_initial,
                )
                    .in_set(LightingSet::Enqueue),
                ApplyDeferred,
                (
                    propagate_decrease_block_system,
                    propagate_decrease_sky_system,
                )
                    .in_set(LightingSet::PropagateDecrease),
                ApplyDeferred,
                (
                    propagate_increase_block_system,
                    propagate_increase_sky_system,
                )
                    .in_set(LightingSet::PropagateIncrease),
            )
                .chain(),
        );
    }
}

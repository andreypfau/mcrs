use crate::codec::{
    emit_column_light_updates, BlockLightDirty, ColumnLightUpdate, SkyLightDirty,
};
use crate::converge::{
    light_converge_driver, set_tick_start, LightConvergeSchedule, LightConvergeSet, TickStart,
};
use crate::distribute::{distribute_decrease, distribute_increase};
use crate::emit_dirty::{
    clear_light_dirty_safety_net, clear_light_tickets, downgrade_light_storage,
    emit_block_light_dirty, emit_sky_light_dirty,
};
use crate::enqueue::{
    consume_needs_full_reseed, enqueue_block_light_on_block_placed, enqueue_sky_light_initial,
    enqueue_sky_light_on_block_placed, pull_neighbor_edge_levels, seed_initial_light,
};
use crate::heightmap_update::update_heightmaps_on_block_placed;
use crate::lifecycle::{attach_lighting_state, prime_heightmaps_on_column_spawn};
use crate::propagate::{
    propagate_decrease_block_system, propagate_decrease_sky_system,
    propagate_increase_block_system, propagate_increase_sky_system,
};
use crate::sets::LightingSet;
use crate::table::build_block_light_table;
use bevy_app::{App, FixedPostUpdate, FixedUpdate, Plugin};
use bevy_ecs::prelude::{ApplyDeferred, IntoScheduleConfigs};
use bevy_ecs::schedule::{ExecutorKind, Schedule};
use bevy_state::prelude::OnEnter;
use mcrs_core::AppState;
use mcrs_engine::world::column::ChunkColumnLifecycleSet;
use mcrs_minecraft_block::block_update::{apply_set_block_request, BlockPlaced, BlockUpdateSet};
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

        app.init_resource::<crate::table::BlockLightTable>();
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

        // Sub-schedule registration. `add_schedule` takes a `Schedule` value,
        // so build an empty `Schedule::new(label)` first. The single-threaded
        // executor matches the codebase convention for deterministic test
        // playback and avoids the executor's parallel-stage overhead for a
        // schedule with at most four stage groups.
        app.add_schedule(Schedule::new(LightConvergeSchedule));
        #[cfg(debug_assertions)]
        app.edit_schedule(LightConvergeSchedule, |schedule| {
            schedule.set_executor_kind(ExecutorKind::SingleThreaded);
        });

        app.insert_resource(TickStart::default());

        app.add_systems(
            FixedUpdate,
            set_tick_start.before(BlockUpdateSet::ApplyChanges),
        );

        // WorldgenIngestSet::ProcessCompletedColumns runs in FixedPreUpdate per
        // mcrs_minecraft's chunk plugin; Bevy executes FixedPreUpdate strictly
        // before FixedUpdate within FixedMain, so it is omitted from this chain.
        //
        // `configure_sets` only accepts `SystemSet` values; `ApplyDeferred` is a
        // system, not a set, so the inter-set deferred-command flushes are
        // declared via `add_systems` calls below. The set ordering itself lives
        // in this single chain so readers can see the per-tick stage sequence
        // in one place.
        app.configure_sets(
            FixedUpdate,
            (
                ChunkColumnLifecycleSet::Reconcile,
                ChunkColumnLifecycleSet::ReconcileIndex,
                ChunkColumnLifecycleSet::PrimeHeightmaps,
                ChunkColumnLifecycleSet::AttachState,
                BlockUpdateSet::ApplyChanges,
                LightingSet::Enqueue,
                LightingSet::Converge,
                LightingSet::EmitDirty,
            )
                .chain(),
        );

        // The CROSS-08 chain places strict deferred-command flush barriers
        // between each set so consumers see the latest world state. Bevy 0.18
        // auto-inserts sync points at edges with deferred params, but the
        // explicit barriers keep the dependency obvious in the schedule graph
        // and survive future refactors that might split the consuming systems
        // across set boundaries.
        app.add_systems(
            FixedUpdate,
            (
                ApplyDeferred
                    .after(ChunkColumnLifecycleSet::AttachState)
                    .before(BlockUpdateSet::ApplyChanges),
                ApplyDeferred
                    .after(BlockUpdateSet::ApplyChanges)
                    .before(LightingSet::Enqueue),
                ApplyDeferred
                    .after(LightingSet::Enqueue)
                    .before(LightingSet::Converge),
            ),
        );

        // Inner convergence sub-schedule: four stages separated by three
        // ApplyDeferred barriers. No leading or trailing barrier — the outer
        // chain flushes upstream commands before the driver invokes the
        // sub-schedule, and Bevy's end-of-schedule semantics flush deferred
        // commands before the next outer-schedule observation.
        app.add_systems(
            LightConvergeSchedule,
            (
                (
                    propagate_decrease_block_system,
                    propagate_decrease_sky_system,
                )
                    .in_set(LightConvergeSet::PropagateDecrease),
                ApplyDeferred,
                distribute_decrease.in_set(LightConvergeSet::DistributeDecrease),
                ApplyDeferred,
                (
                    propagate_increase_block_system,
                    propagate_increase_sky_system,
                )
                    .in_set(LightConvergeSet::PropagateIncrease),
                ApplyDeferred,
                distribute_increase.in_set(LightConvergeSet::DistributeIncrease),
            )
                .chain(),
        );

        app.configure_sets(
            LightConvergeSchedule,
            (
                LightConvergeSet::PropagateDecrease,
                LightConvergeSet::DistributeDecrease,
                LightConvergeSet::PropagateIncrease,
                LightConvergeSet::DistributeIncrease,
            )
                .chain(),
        );

        // Enqueue stage: six systems. `seed_initial_light` runs strictly
        // before `pull_neighbor_edge_levels` so the just-loaded section's own
        // emitter/sky seeds land first, then the neighbour-edge merge layers
        // on top. The other systems are unordered relative to one another
        // (their queries are disjoint).
        app.add_systems(
            FixedUpdate,
            (
                enqueue_block_light_on_block_placed,
                enqueue_sky_light_on_block_placed,
                enqueue_sky_light_initial,
                consume_needs_full_reseed,
                seed_initial_light,
                pull_neighbor_edge_levels.after(seed_initial_light),
            )
                .in_set(LightingSet::Enqueue),
        );

        app.add_systems(
            FixedUpdate,
            light_converge_driver.in_set(LightingSet::Converge),
        );

        app.add_systems(
            FixedUpdate,
            (
                downgrade_light_storage,
                emit_block_light_dirty,
                emit_sky_light_dirty,
                clear_light_dirty_safety_net,
                clear_light_tickets,
            )
                .chain()
                .in_set(LightingSet::EmitDirty),
        );

        app.add_message::<BlockLightDirty>();
        app.add_message::<SkyLightDirty>();
        app.add_message::<ColumnLightUpdate>();
        app.configure_sets(FixedPostUpdate, LightingSet::Codec);
        app.add_systems(
            FixedPostUpdate,
            emit_column_light_updates.in_set(LightingSet::Codec),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::schedule::Schedules;
    use bevy_state::app::{AppExtStates, StatesPlugin};
    use mcrs_engine::world::column::ColumnPlugin;

    fn build_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(StatesPlugin);
        app.init_state::<AppState>();
        app.add_plugins(ColumnPlugin);
        app.add_plugins(LightingPlugin);
        app
    }

    #[test]
    fn light_converge_schedule_registered() {
        let app = build_test_app();
        let schedules = app.world().resource::<Schedules>();
        assert!(
            schedules.contains(LightConvergeSchedule),
            "LightConvergeSchedule not registered by LightingPlugin"
        );
    }

    #[test]
    fn light_converge_set_chain_configured() {
        let app = build_test_app();
        let schedules = app.world().resource::<Schedules>();
        let schedule = schedules
            .get(LightConvergeSchedule)
            .expect("LightConvergeSchedule registered");
        assert!(
            schedule.systems_len() >= 4,
            "LightConvergeSchedule should contain at least four systems; got {}",
            schedule.systems_len()
        );
    }
}

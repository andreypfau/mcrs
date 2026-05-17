use crate::block_light::BlockLightPlugin;
use crate::codec::{emit_column_light_updates, ColumnLightUpdate};
use crate::sky_light::SkyLightPlugin;
use crate::converge::{
    light_converge_driver, LightConvergeSchedule, LightConvergeSet,
};
use crate::distribute::{distribute_block_wavefronts, distribute_sky_wavefronts};
use crate::emit_dirty::{
    clear_light_tickets,
    downgrade_light_storage,
};
use crate::enqueue::consume_needs_full_reseed;
use crate::sky_light::enqueue::invalidate_previous_topmost;
use crate::heightmap_update::update_heightmaps_on_block_placed;
use crate::lifecycle::{attach_lighting_state, prime_heightmaps_on_column_spawn};
use crate::sets::LightingSet;
use crate::table::build_block_light_table;
use bevy_app::{App, FixedPostUpdate, FixedUpdate, Plugin};
use bevy_ecs::prelude::{ApplyDeferred, IntoScheduleConfigs};
use bevy_ecs::schedule::{ExecutorKind, Schedule};
use bevy_state::prelude::OnEnter;
use mcrs_core::AppState;
use mcrs_engine::world::column::ColumnLifecycleSet;
use mcrs_minecraft_block::block_update::{apply_set_block_request, BlockPlaced, BlockUpdateSet};
use mcrs_vanilla::{freeze_static_tags, transition_to_playing};
use crate::block_light::emit_dirty::{clear_block_bfs_pending_safety_net, emit_block_light_dirty};
use crate::block_light::enqueue::{enqueue_block_light_on_block_placed, pull_block_neighbor_edges, seed_block_emitters};
use crate::block_light::propagate::{propagate_decrease_block_system, propagate_increase_block_system};
use crate::sky_light::emit_dirty::{clear_sky_bfs_pending_safety_net, emit_sky_light_dirty};
use crate::sky_light::enqueue::{enqueue_sky_light_on_block_placed, pull_sky_neighbor_edges, seed_sky_initial};
use crate::sky_light::propagate::{propagate_decrease_sky_system, propagate_increase_sky_system};

#[cfg(feature = "lighting-trace")]
fn span_lighting_enqueue() {
    let _span = tracing::info_span!("lighting::enqueue").entered();
}

#[cfg(feature = "lighting-trace")]
fn span_lighting_converge() {
    let _span = tracing::info_span!("lighting::converge").entered();
}

#[cfg(feature = "lighting-trace")]
fn span_lighting_emit_dirty() {
    let _span = tracing::info_span!("lighting::emit_dirty").entered();
}

#[cfg(feature = "lighting-trace")]
fn span_lighting_codec() {
    let _span = tracing::info_span!("lighting::codec").entered();
}

pub struct LightingPlugin;

impl Plugin for LightingPlugin {
    fn build(&self, app: &mut App) {
        // `update_heightmaps_on_block_placed` reads `MessageReader<BlockPlaced>`.
        // The production binary also registers `BlockUpdatePlugin`, which calls
        // `add_message::<BlockPlaced>()`. Registering twice would re-initialize
        // the message buffer and drop parked messages, so guard against the
        // duplicate so the lighting plugin stays self-contained for integration
        // tests but no-ops when `BlockUpdatePlugin` has already initialized the
        // buffer.
        if !app
            .world()
            .contains_resource::<bevy_ecs::message::Messages<BlockPlaced>>()
        {
            app.add_message::<BlockPlaced>();
        }

        app.init_resource::<crate::table::BlockStateLightTable>();
        app.add_systems(
            OnEnter(AppState::WorldgenFreeze),
            build_block_light_table
                .after(freeze_static_tags)
                .before(transition_to_playing),
        );

        // Cross-plugin barrier: the chain begins with a leading `ApplyDeferred`
        // so the spawn `Commands` queued upstream by `reconcile_column_chunks`
        // are flushed before the heightmap-prime query reads `Column` and
        // `ColumnChunks` state. The upstream `ColumnPlugin` intentionally
        // omits a trailing post-reconcile `ApplyDeferred`; the responsibility
        // lives here on the consumer side.
        app.add_systems(
            FixedUpdate,
            (
                ApplyDeferred,
                prime_heightmaps_on_column_spawn
                    .in_set(ColumnLifecycleSet::PrimeHeightmaps),
                ApplyDeferred,
                attach_lighting_state.in_set(ColumnLifecycleSet::AttachState),
            )
                .chain()
                .after(ColumnLifecycleSet::ReconcileIndex),
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
                ColumnLifecycleSet::Reconcile,
                ColumnLifecycleSet::ReconcileIndex,
                ColumnLifecycleSet::PrimeHeightmaps,
                ColumnLifecycleSet::AttachState,
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
                    .after(ColumnLifecycleSet::AttachState)
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
                (distribute_block_wavefronts, distribute_sky_wavefronts)
                    .in_set(LightConvergeSet::DistributeDecrease),
                ApplyDeferred,
                (
                    propagate_increase_block_system,
                    propagate_increase_sky_system,
                )
                    .in_set(LightConvergeSet::PropagateIncrease),
                ApplyDeferred,
                (distribute_block_wavefronts, distribute_sky_wavefronts)
                    .in_set(LightConvergeSet::DistributeIncrease),
                ApplyDeferred,
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

        // Enqueue stage: the just-loaded chunk's own emitter/sky seeds land
        // first via the parallel `(seed_block_emitters, seed_sky_initial)`
        // pair (disjoint queues queries — slotted in parallel by Bevy's
        // conflict graph). `invalidate_previous_topmost` runs after
        // `seed_sky_initial` so the `NeedsRetop` handoff is visible across
        // the `apply_deferred` barrier between Enqueue substages. The
        // per-channel neighbour-edge merge layers on top via the
        // `(pull_block_neighbor_edges, pull_sky_neighbor_edges)` tuple,
        // reanchored to `seed_sky_initial` so the sky-side pull's
        // `Uniform(15)`-neighbour observation depends on the Case A
        // heightmap fast-path writes from `seed_sky_initial`.
        app.add_systems(
            FixedUpdate,
            (
                enqueue_block_light_on_block_placed,
                enqueue_sky_light_on_block_placed,
                consume_needs_full_reseed,
                (seed_block_emitters, seed_sky_initial),
                invalidate_previous_topmost.after(seed_sky_initial),
                (pull_block_neighbor_edges, pull_sky_neighbor_edges)
                    .after(seed_sky_initial),
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
                (emit_block_light_dirty, emit_sky_light_dirty),
                (
                    clear_block_bfs_pending_safety_net,
                    clear_sky_bfs_pending_safety_net,
                ),
                clear_light_tickets,
            )
                .chain()
                .in_set(LightingSet::EmitDirty),
        );

        app.add_plugins((BlockLightPlugin, SkyLightPlugin));
        app.add_message::<ColumnLightUpdate>();
        app.configure_sets(FixedPostUpdate, LightingSet::Codec);
        app.add_systems(
            FixedPostUpdate,
            emit_column_light_updates.in_set(LightingSet::Codec),
        );

        #[cfg(feature = "lighting-trace")]
        app.add_systems(
            FixedUpdate,
            span_lighting_enqueue
                .in_set(LightingSet::Enqueue)
                .before(seed_block_emitters),
        );
        #[cfg(feature = "lighting-trace")]
        app.add_systems(
            FixedUpdate,
            span_lighting_converge
                .in_set(LightingSet::Converge)
                .before(light_converge_driver),
        );
        #[cfg(feature = "lighting-trace")]
        app.add_systems(
            FixedUpdate,
            span_lighting_emit_dirty
                .in_set(LightingSet::EmitDirty)
                .before(downgrade_light_storage),
        );
        #[cfg(feature = "lighting-trace")]
        app.add_systems(
            FixedPostUpdate,
            span_lighting_codec
                .in_set(LightingSet::Codec)
                .before(emit_column_light_updates),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::prelude::SystemSet;
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

    #[test]
    fn emit_systems_register_in_parallel_after_downgrade() {
        let app = build_test_app();
        let schedules = app.world().resource::<Schedules>();
        let schedule = schedules
            .get(FixedUpdate)
            .expect("FixedUpdate schedule registered");
        let graph = schedule.graph();

        let find_key = |needle: &str| -> bevy_ecs::schedule::SystemKey {
            graph
                .systems
                .iter()
                .find_map(|(key, system, _conditions)| {
                    format!("{}", system.name()).contains(needle).then_some(key)
                })
                .unwrap_or_else(|| panic!("system `{needle}` not found in FixedUpdate"))
        };

        let block_key = find_key("emit_block_light_dirty");
        let sky_key = find_key("emit_sky_light_dirty");
        let downgrade_key = find_key("downgrade_light_storage");

        let emit_dirty_set_key = graph
            .system_sets
            .get_key(LightingSet::EmitDirty.intern())
            .expect("LightingSet::EmitDirty registered as a SystemSet");
        let emit_dirty_node: bevy_ecs::schedule::NodeId = emit_dirty_set_key.into();

        let hierarchy = graph.hierarchy().graph();
        for (member, label) in [
            (block_key, "emit_block_light_dirty"),
            (sky_key, "emit_sky_light_dirty"),
            (downgrade_key, "downgrade_light_storage"),
        ] {
            assert!(
                hierarchy.contains_edge(emit_dirty_node, member.into()),
                "{label} should sit inside LightingSet::EmitDirty"
            );
        }

        let dependency = graph.dependency().graph();
        let edge_between_emit = dependency.contains_edge(block_key.into(), sky_key.into())
            || dependency.contains_edge(sky_key.into(), block_key.into());
        assert!(
            !edge_between_emit,
            "emit_block_light_dirty and emit_sky_light_dirty should have no required ordering edge — the inner tuple slots them in parallel"
        );

        assert!(
            dependency.contains_edge(downgrade_key.into(), block_key.into()),
            "outer chain should order downgrade_light_storage before emit_block_light_dirty"
        );
        assert!(
            dependency.contains_edge(downgrade_key.into(), sky_key.into()),
            "outer chain should order downgrade_light_storage before emit_sky_light_dirty"
        );
    }
}

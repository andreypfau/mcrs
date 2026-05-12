use crate::lifecycle::{attach_lighting_state, prime_heightmaps_on_column_spawn};
use crate::table::build_block_light_table;
use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::prelude::{ApplyDeferred, IntoScheduleConfigs};
use bevy_state::prelude::OnEnter;
use mcrs_core::AppState;
use mcrs_engine::world::column::ChunkColumnLifecycleSet;
use mcrs_vanilla::{freeze_static_tags, transition_to_playing};

pub struct LightingPlugin;

impl Plugin for LightingPlugin {
    fn build(&self, app: &mut App) {
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
    }
}

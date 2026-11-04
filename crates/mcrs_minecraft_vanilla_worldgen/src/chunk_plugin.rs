use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{
    Local, MessageReader, ParamSet, Query, Resource, With, resource_exists_and_changed,
};
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::system::{Commands, Res, ResMut};
use bevy_tasks::{TaskPool, TaskPoolBuilder};
use mcrs_engine_link::{Link, Placement};
use mcrs_minecraft_engine::column_store::StoredColumnLandSet;
use mcrs_minecraft_engine::heightmap::PendingColumnHeightmaps;
use mcrs_minecraft_engine_world::aoi::min_column_distance;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::entity::player::chunk_view::SentChunkView;
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_engine_world::world::dimension::InDimension;
use mcrs_minecraft_engine_world::world::lifecycle::stage::{
    SectionStage, SectionStageChanged, SectionStages,
};
use mcrs_minecraft_engine_world::world::lifecycle::ticket::ChunkSpawnSet;
use mcrs_minecraft_engine_world::world::lifecycle::trace as column_trace;
use mcrs_minecraft_engine_world::world::lifecycle::trace::{ColumnStage, ColumnTraceLog};
use mcrs_minecraft_engine_world::world::storage::block_entity::spawn_block_entities;
use mcrs_minecraft_simulation::PendingTick;
use mcrs_minecraft_worldgen_feature::placer::GenerationKinds;
use mcrs_minecraft_worldgen_generator::stages::FillContext;
use mcrs_minecraft_worldgen_generator::task::ColumnSource;
use mcrs_voxel::{ColumnPos, SectionPos};
use rustc_hash::FxHashMap;
use tracing::{info, trace};

use crate::generation_input::{GenerationInput, RequestedSection, ViewUpdate, WantedColumn};
use crate::generation_link::GenerationLink;
use crate::generation_output::{GeneratedColumn, GenerationOutput, SectionContent};
use crate::generation_stage::{GenerationStage, SchedulerConfig};

/// The generation of one dimension, by a generation stage at the far end of a
/// [`GenerationLink`].
///
/// The stage starts empty: the [`FillContext`] reaches it as an input, and requests sent
/// before it wait inside the stage.
pub(crate) struct ChunkPlugin {
    /// Where the stage runs. [`Placement::Inline`] generates everything requested in a tick at
    /// the start of the next one, on the thread of the dimension.
    pub(crate) placement: Placement,
}

impl Plugin for ChunkPlugin {
    fn build(&self, app: &mut App) {
        let config = SchedulerConfig::default();
        CHUNK_TASK_POOL.get_or_init(|| {
            TaskPoolBuilder::new()
                .thread_name("ChunkGen".to_string())
                .num_threads(config.num_threads)
                .build()
        });
        let traces = app.world().get_resource::<ColumnTraceLog>().cloned();
        let link = Link::new(GenerationStage::new(traces, config), self.placement);
        app.insert_resource(GenerationLink::new(link));
        app.init_resource::<ColumnLatency>();
        // The chunks a view ticketed this tick are spawned this tick and requested right
        // after. What the stage generated since lands at the start of the next tick.
        app.add_systems(
            FixedUpdate,
            (
                integrate_generated_columns
                    .in_set(TickSet::DomainInput)
                    .after(StoredColumnLandSet),
                (
                    send_fill_context.run_if(resource_exists_and_changed::<FillContext>),
                    request_columns,
                    send_views,
                )
                    .chain()
                    .in_set(TickSet::Project)
                    .after(ChunkSpawnSet),
            ),
        );
    }
}

pub(crate) static CHUNK_TASK_POOL: OnceLock<TaskPool> = OnceLock::new();

pub(crate) fn send_fill_context(ctx: Res<FillContext>, mut generation: ResMut<GenerationLink>) {
    generation.send(GenerationInput::Context(Box::new(ctx.clone())));
}

/// Hands the generation stage the columns a view asked for.
pub(crate) fn request_columns(
    mut stages: ParamSet<(MessageReader<SectionStageChanged>, SectionStages)>,
    mut requested: Local<Vec<(Entity, SectionPos)>>,
    mut generation: ResMut<GenerationLink>,
    mut latency: ResMut<ColumnLatency>,
    players: Query<&Transform, With<Player>>,
    mut columns: Local<HashMap<ColumnPos, Vec<RequestedSection>>>,
    mut player_positions: Local<Vec<ColumnPos>>,
) {
    requested.extend(
        stages
            .p0()
            .read()
            .filter(|change| change.to == SectionStage::Loading)
            .map(|change| (change.section, change.pos)),
    );
    let mut section_stages = stages.p1();
    for (section, pos) in requested.drain(..) {
        if section_stages.get(section) != Some(SectionStage::Loading) {
            continue;
        }
        trace!(
            "Requested generation for chunk section at ({}, {}, {})",
            pos.x, pos.y, pos.z
        );
        section_stages.set(section, SectionStage::Generating);
        columns
            .entry(ColumnPos::new(pos.x, pos.z))
            .or_default()
            .push(RequestedSection {
                entity: section,
                y: pos.y,
            });
    }
    if columns.is_empty() {
        return;
    }

    player_positions.clear();
    player_positions.extend(players.iter().map(|t| ColumnPos::from(t.translation)));
    for (col, sections) in columns.drain() {
        latency.requested.entry(col).or_insert_with(Instant::now);
        generation.send(GenerationInput::Want(WantedColumn {
            col,
            distance_sq: min_column_distance(&col, &player_positions),
            sections,
        }));
    }
}

/// Tells the generation stage what the views and the players are, so it drops what left every
/// view and works the nearest columns first.
pub(crate) fn send_views(
    mut generation: ResMut<GenerationLink>,
    views: Query<&SentChunkView>,
    players: Query<&Transform, With<Player>>,
) {
    generation.send(GenerationInput::Views(ViewUpdate {
        views: views.iter().filter_map(|view| view.0).collect(),
        players: players
            .iter()
            .map(|transform| ColumnPos::from(transform.translation))
            .collect(),
    }));
}

/// Puts what the generation stage generated into the sections that asked for it, and sends
/// the sections of the columns it dropped on their way out.
///
/// The pool finishes columns in an order its threads decide, so the outputs are integrated
/// by position: the entities and messages they produce do not depend on that order.
#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system: each resource and query is its own parameter so the scheduler sees the access"
)]
pub(crate) fn integrate_generated_columns(
    mut generation: ResMut<GenerationLink>,
    mut generated: Local<Vec<GeneratedColumn>>,
    mut abandoned: Local<Vec<Entity>>,
    mut pending_heightmaps: ResMut<PendingColumnHeightmaps>,
    section_dimensions: Query<&InDimension>,
    mut stages: SectionStages,
    mut commands: Commands,
    slow_threshold: Option<Res<SlowColumnThreshold>>,
    mut latency: ResMut<ColumnLatency>,
    traces: Option<Res<ColumnTraceLog>>,
) {
    let threshold = slow_threshold.as_deref().copied().unwrap_or_default().0;
    for output in generation.outputs() {
        match output {
            GenerationOutput::Generated(column) => generated.push(*column),
            GenerationOutput::Abandoned(column) => {
                latency.requested.remove(&column.col);
                abandoned.extend(column.sections);
            }
        }
    }
    abandoned.sort_unstable();
    for section in abandoned.drain(..) {
        if stages.get(section) == Some(SectionStage::Generating) {
            stages.set(section, SectionStage::Unloading);
        }
    }
    // Stable, so two outputs of one column keep the order the stage emitted them in.
    generated.sort_by_key(|column| column.col);
    for column in generated.drain(..) {
        let GeneratedColumn {
            col,
            source,
            maps,
            block_entities,
            entities,
            sections,
        } = column;
        // Any live section of the column answers for the whole column. The
        // first-requested one may already have been despawned while the rest of
        // the column still stands, and taking it alone would drop the lot.
        let dim = sections
            .iter()
            .find_map(|section| section_dimensions.get(section.entity).ok())
            .copied();
        match dim {
            Some(dim) => {
                // No plugin spawns generated entities yet, so they are dropped here.
                spawn_block_entities(&mut commands, dim, block_entities);
            }
            None => tracing::debug!(
                dropped = block_entities.len() + entities.len(),
                "a column with no live section landed its entities nowhere"
            ),
        }

        latency.report(threshold, col, sections.len(), source);
        column_trace::mark(traces.as_deref(), col, ColumnStage::Loaded);
        column_trace::set_source(traces.as_deref(), col, source.label());
        if let (Some(dim), Some(maps)) = (dim, maps) {
            pending_heightmaps.0.insert((dim.0, col), maps);
        }

        for section in sections {
            let entity = section.entity;
            match section.content {
                SectionContent::Blocks(content) => {
                    let mut commands = commands.entity(entity);
                    commands.try_insert((content.blocks, content.biomes));
                    if !content.ticks.is_empty() {
                        commands.try_insert(GeneratedTicks(content.ticks));
                    }
                    // One cancelled while it generated keeps the blocks for a ticket that
                    // takes it back, but stays on its way out.
                    if stages.get(entity) == Some(SectionStage::Generating) {
                        stages.set(entity, SectionStage::Loaded);
                    }
                }
                SectionContent::Unchecked => {
                    tracing::error!(?col, "a section arrived with no block table to check it");
                    stages.set(entity, SectionStage::Unloading);
                }
                SectionContent::Missing => stages.set(entity, SectionStage::Unloading),
            }
        }
    }
}

/// A column slower than this from its request to its landing gets a line in the log.
#[derive(Resource, Clone, Copy, Debug)]
pub(crate) struct SlowColumnThreshold(pub(crate) Duration);

impl Default for SlowColumnThreshold {
    fn default() -> Self {
        Self(Duration::from_millis(250))
    }
}

/// When the dimension first asked for each column it waits on, for the slow-column log only.
/// The wall clock read here reaches neither the generation stage nor the simulation.
#[derive(Resource, Default)]
pub(crate) struct ColumnLatency {
    requested: FxHashMap<ColumnPos, Instant>,
    reported: Option<Instant>,
    unreported: u64,
}

impl ColumnLatency {
    /// The whole ladder's latency, reported once a second with the count of the
    /// columns the sample stands for.
    fn report(
        &mut self,
        threshold: Duration,
        col: ColumnPos,
        sections: usize,
        source: ColumnSource,
    ) {
        let Some(requested) = self.requested.remove(&col) else {
            return;
        };
        let total = requested.elapsed();
        if total < threshold {
            return;
        }
        // A stall is a property of the whole load, not of one column, and a line per
        // column drowns out every other log on the server.
        self.unreported = self.unreported.saturating_add(1);
        if self
            .reported
            .is_some_and(|at| at.elapsed() < Duration::from_secs(1))
        {
            return;
        }
        let others = std::mem::replace(&mut self.unreported, 0).saturating_sub(1);
        self.reported = Some(Instant::now());
        info!(
            x = col.x,
            z = col.z,
            source = source.label(),
            total_ms = total.as_secs_f32() * 1000.0,
            sections = sections,
            others = others,
            "slow chunk column"
        );
    }
}

/// The ticks generation scheduled in a section, in the order it asked, held for the
/// simulation that runs the section.
#[derive(bevy_ecs::prelude::Component, Clone, Debug, PartialEq)]
pub(crate) struct GeneratedTicks(pub Vec<PendingTick<GenerationKinds>>);

#[cfg(test)]
pub(crate) fn request_section(
    world: &mut bevy_ecs::world::World,
    dim: Entity,
    pos: SectionPos,
) -> Entity {
    let section = world
        .spawn((pos, InDimension(dim), SectionStage::Loading))
        .id();
    world.write_message(SectionStageChanged::spawned(
        section,
        pos,
        dim,
        SectionStage::Loading,
    ));
    section
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_app::Update;
    use bevy_tasks::block_on;
    use mcrs_minecraft_engine_world::entity::player::chunk_view::ChunkTrackingView;
    use mcrs_minecraft_engine_world::palette::ChunkBlocks;
    use mcrs_minecraft_worldgen_generator::task::CancellationToken;
    use std::sync::Arc;

    fn generation_app(placement: Placement, config: SchedulerConfig) -> App {
        let mut app = App::new();
        app.add_message::<SectionStageChanged>();
        app.init_resource::<PendingColumnHeightmaps>();
        app.init_resource::<ColumnLatency>();
        let link = Link::new(GenerationStage::new(None, config), placement);
        app.insert_resource(GenerationLink::new(link));
        app
    }

    /// One tick that requests, a wait for the stage, and one tick that lands what it generated.
    fn tick_and_land(app: &mut App) {
        app.update();
        block_on(app.world_mut().resource_mut::<GenerationLink>().answered());
        app.update();
    }

    /// The ladder must produce the column the fill alone produces: with no
    /// features to run, every delta is empty and the merge is the identity.
    /// This drives the whole of it — the derived 5x5 of fills, the 3x3 of runs,
    /// the merge and the emission — through each placement of the stage, so a
    /// readiness or budget bug shows up as a column that never arrives.
    #[test]
    fn the_ladder_generates_the_column_the_fill_produced() {
        for placement in [Placement::Inline, Placement::Thread] {
            the_ladder_generates_the_filled_column(placement);
        }
    }

    fn the_ladder_generates_the_filled_column(placement: Placement) {
        use mcrs_minecraft_worldgen_generator::stages::fill_column;
        use mcrs_minecraft_worldgen_generator::tests::build_beta_router;

        CHUNK_TASK_POOL.get_or_init(|| TaskPoolBuilder::new().num_threads(4).build());

        let router = Arc::new(build_beta_router());
        let col = ColumnPos::new(1, -2);
        let ctx = mcrs_minecraft_worldgen_generator::tests::bare_fill_context(router);
        let y_sections = ctx.y_sections.clone();

        let mut app = generation_app(
            placement,
            SchedulerConfig {
                max_in_flight: 64,
                max_dispatch_per_pass: 64,
                num_threads: 2,
            },
        );
        app.insert_resource(ctx.clone());
        app.insert_resource(mcrs_minecraft_item::Items(Arc::default()));
        app.add_systems(
            Update,
            (
                send_fill_context.run_if(resource_exists_and_changed::<FillContext>),
                request_columns,
                integrate_generated_columns,
            )
                .chain(),
        );

        let dim = app.world_mut().spawn_empty().id();
        let carried: Vec<i32> = vec![-1, 0, 4];
        let sections: Vec<Entity> = carried
            .iter()
            .map(|&y| request_section(app.world_mut(), dim, SectionPos::new(col.x, y, col.z)))
            .collect();
        let loaded = |app: &App, section: Entity| {
            app.world().get::<SectionStage>(section) == Some(&SectionStage::Loaded)
        };

        tick_and_land(&mut app);
        assert!(
            sections.iter().all(|e| loaded(&app, *e)),
            "{placement:?}: the ladder never generated the column"
        );

        let mut buffer = mcrs_minecraft_worldgen_generator::ColumnBlocks::new(&y_sections);
        let alone = fill_column(&ctx, col, &mut buffer, &CancellationToken::new())
            .expect("the fill was not cancelled");
        let bottom = y_sections[0];

        // A section ticketed after the column was generated must come from the
        // same merged blocks, not from the snapshot the neighbours read.
        let late = request_section(app.world_mut(), dim, SectionPos::new(col.x, 5, col.z));
        tick_and_land(&mut app);
        assert!(
            loaded(&app, late),
            "{placement:?}: a section ticketed after the column was generated never landed"
        );

        for (&y, entity) in carried
            .iter()
            .chain([&5])
            .zip(sections.iter().chain([&late]))
        {
            let want = alone.sections[(y - bottom) as usize]
                .as_ref()
                .expect("the fill produced this section");
            let got = app
                .world()
                .get::<ChunkBlocks>(*entity)
                .expect("a generated section carries its blocks");
            for cell in 0..mcrs_minecraft_worldgen_generator::ColumnBlocks::SECTION_VOLUME {
                let local = mcrs_voxel::LocalPos::from_index(cell);
                let (x, ly, z) = (local.x() as usize, local.y() as usize, local.z() as usize);
                assert_eq!(
                    got.get_cell(x, ly, z)
                        .map(mcrs_minecraft_chunk::VoxelId::from),
                    want.0.get_cell(x, ly, z),
                    "{placement:?}: section {y} diverged at ({x}, {ly}, {z})"
                );
            }
        }
    }

    #[test]
    fn a_column_that_left_every_view_unloads_its_sections() {
        let mut app = generation_app(Placement::Inline, SchedulerConfig::default());
        app.add_systems(Update, integrate_generated_columns);

        let dim = app.world_mut().spawn_empty().id();
        let stale_pos = SectionPos::new(100, 0, 100);
        let stale_section = app
            .world_mut()
            .spawn((stale_pos, InDimension(dim), SectionStage::Generating))
            .id();
        let mut generation = app.world_mut().resource_mut::<GenerationLink>();
        generation.send(GenerationInput::Want(WantedColumn {
            col: ColumnPos::new(stale_pos.x, stale_pos.z),
            distance_sq: 0,
            sections: vec![RequestedSection {
                entity: stale_section,
                y: stale_pos.y,
            }],
        }));
        generation.send(GenerationInput::Views(ViewUpdate {
            views: vec![ChunkTrackingView {
                center: SectionPos::new(0, 0, 0),
                distance: 2,
                vert_distance: 8,
                min_section_y: i32::MIN,
                max_section_y: i32::MAX,
            }],
            players: Vec::new(),
        }));
        app.update();

        assert_eq!(
            app.world().get::<SectionStage>(stale_section),
            Some(&SectionStage::Unloading),
            "a stale section is sent on its way out"
        );
    }
}

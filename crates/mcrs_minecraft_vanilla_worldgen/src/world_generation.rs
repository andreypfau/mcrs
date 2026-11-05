use std::collections::BTreeMap;
use std::mem;

use bevy_ecs::entity::Entity;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::{Local, ResMut};
use bevy_ecs::world::World;
use mcrs_engine_cell::CellId;
use mcrs_engine_link::{Link, Placement};
use mcrs_minecraft_engine::column_generation::GenerationAnswer;
use mcrs_minecraft_engine::coordinator::dim_cells::DimCells;
use mcrs_minecraft_engine::foreign_views::ForeignViews;
use mcrs_minecraft_engine_world::aoi::min_column_distance;

use mcrs_minecraft_engine_world::world::dimension_builders::DimensionBuild;
use mcrs_minecraft_worldgen_generator::stages::FillContext;
use mcrs_voxel::{ColumnPos, SectionPos};
use rustc_hash::FxHashMap;

use crate::generation_input::{GenerationInput, ViewUpdate, WantedColumn};
use crate::generation_stage::{GenerationStage, SchedulerConfig, init_chunk_task_pool};

/// The generation of each world, in the host: one stage for each world generates the whole
/// columns that every cell of the world asks for, for its hot sections and for its data layer
/// alike. A column and the lower levels around it are thus generated once for the world.
///
/// A cell asks in the output of a tick, and the answer goes to each cell that asked in the input
/// of the next tick after the stage finished the column, as the data layer answers. A world of
/// several cells thus gets each column in the same tick as a world of one cell.
#[derive(Resource, Default)]
pub struct WorldGeneration {
    worlds: BTreeMap<Entity, WorldStage>,
}

struct WorldStage {
    link: Link<GenerationStage>,
    /// The cells that wait on each column the stage generates.
    askers: FxHashMap<ColumnPos, Vec<CellId>>,
    /// The view of each player of the world.
    views: ForeignViews,
    views_changed: bool,
}

impl WorldGeneration {
    /// Waits until the stage of every world has answered every column asked so far, and hands
    /// out none of them.
    pub async fn answered(&mut self) {
        for stage in self.worlds.values_mut() {
            stage.link.answered().await;
        }
    }
}

impl WorldStage {
    fn ask(&mut self, cell: CellId, column: ColumnPos, players: &[ColumnPos]) {
        let askers = self.askers.entry(column).or_default();
        if askers.is_empty() {
            self.link.send(GenerationInput::Want(WantedColumn {
                col: column,
                distance_sq: min_column_distance(&column, players),
            }));
        }
        if !askers.contains(&cell) {
            askers.push(cell);
        }
    }

    fn players(&self) -> Vec<ColumnPos> {
        self.views
            .views()
            .map(|view| ColumnPos::new(view.center.x, view.center.z))
            .collect()
    }

    fn send_views(&mut self, players: Vec<ColumnPos>) {
        if !mem::take(&mut self.views_changed) {
            return;
        }
        self.link.send(GenerationInput::Views(ViewUpdate {
            views: self.views.views().collect(),
            players,
        }));
    }
}

/// Starts the stage of the world of `label` from the generation context of its first cell.
pub(crate) fn build_world_generation(
    host: &mut World,
    label: Entity,
    build: &DimensionBuild<'_>,
    cell: &World,
) {
    let Some(fill) = cell.get_resource::<FillContext>() else {
        return;
    };
    let first = build.type_config.min_y >> SectionPos::BITS;
    let count = i32::try_from(build.type_config.section_count).unwrap_or(0);
    let config = SchedulerConfig::default();
    init_chunk_task_pool(&config);
    let rows = first..first.saturating_add(count);
    let mut link = Link::new(GenerationStage::new(config, rows), Placement::Thread);
    link.send(GenerationInput::Context(Box::new(fill.clone())));
    host.get_resource_or_init::<WorldGeneration>()
        .worlds
        .insert(
            label,
            WorldStage {
                link,
                askers: FxHashMap::default(),
                views: ForeignViews::default(),
                views_changed: false,
            },
        );
}

/// Hands each stage the columns that the cells of its world asked for in the tick that just
/// ended, then the views of the players of the world.
pub(crate) fn ask_world_columns(
    mut cells: ResMut<DimCells>,
    mut generation: ResMut<WorldGeneration>,
) {
    let generation = &mut *generation;
    generation.worlds.retain(|label, _| cells.contains(*label));
    for reported in cells.reported_views() {
        if let Some(stage) = generation.worlds.get_mut(&reported.label) {
            stage.views_changed |= stage.views.apply(reported.view);
        }
    }
    let asks = cells.take_generation_asks();
    let mut players: BTreeMap<Entity, Vec<ColumnPos>> = BTreeMap::new();
    for ask in asks {
        let Some(stage) = generation.worlds.get_mut(&ask.label) else {
            continue;
        };
        let players = players.entry(ask.label).or_insert_with(|| stage.players());
        stage.ask(ask.cell, ask.column, players);
    }
    for (label, stage) in &mut generation.worlds {
        let players = players.remove(label).unwrap_or_else(|| stage.players());
        stage.send_views(players);
    }
}

/// Hands what each stage generated or dropped to each cell that asked for it, for the tick that
/// starts now. The pool finishes columns in an order its threads decide, so a cell gets them by
/// position.
pub(crate) fn deliver_world_columns(
    mut cells: ResMut<DimCells>,
    mut generation: ResMut<WorldGeneration>,
    mut finished: Local<Vec<GenerationAnswer>>,
) {
    for stage in generation.worlds.values_mut() {
        finished.extend(stage.link.outputs());
        finished.sort_by_key(|answer| answer.column);
        for answer in finished.drain(..) {
            let Some(askers) = stage.askers.remove(&answer.column) else {
                continue;
            };
            let Some((last, rest)) = askers.split_last() else {
                continue;
            };
            for &cell in rest {
                cells.deliver_generated(cell, answer.clone());
            }
            cells.deliver_generated(*last, answer);
        }
    }
}

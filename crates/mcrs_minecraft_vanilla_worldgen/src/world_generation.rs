use std::collections::BTreeMap;
use std::mem;

use bevy_ecs::entity::Entity;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::{Local, Res, ResMut};
use bevy_ecs::world::World;
use mcrs_engine_cell::CellId;
use mcrs_engine_link::{Link, Placement};
use mcrs_minecraft_engine::column_generation::GenerationAnswer;
use mcrs_minecraft_engine::coordinator::dim_cells::DimCells;
use mcrs_minecraft_engine::foreign_views::ForeignViews;

use mcrs_minecraft_engine_world::world::dimension_builders::DimensionBuild;
use mcrs_minecraft_worldgen_generator::stages::FillContext;
use mcrs_voxel::{ColumnPos, SectionPos};
use rustc_hash::FxHashMap;

use crate::generation_input::GenerationInput;
use crate::generation_stage::{GenerationStage, SchedulerConfig, init_chunk_task_pool};

#[derive(Resource, Default)]
pub struct WorldGeneration {
    worlds: BTreeMap<Entity, WorldStage>,
}

struct WorldStage {
    link: Link<GenerationStage>,
    askers: FxHashMap<ColumnPos, Vec<CellId>>,
    views: ForeignViews,
    views_changed: bool,
}

impl WorldGeneration {
    pub async fn answered(&mut self) {
        for stage in self.worlds.values_mut() {
            stage.link.answered().await;
        }
    }
}

impl WorldStage {
    fn ask(&mut self, cell: CellId, column: ColumnPos) {
        let askers = self.askers.entry(column).or_default();
        if askers.is_empty() {
            self.link.send(GenerationInput::Want(column));
        }
        if !askers.contains(&cell) {
            askers.push(cell);
        }
    }

    fn send_views(&mut self) {
        if !mem::take(&mut self.views_changed) {
            return;
        }
        self.link
            .send(GenerationInput::Views(self.views.views().collect()));
    }
}

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

pub(crate) fn generates_worlds(generation: Res<WorldGeneration>) -> bool {
    !generation.worlds.is_empty()
}

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::alloc_in_tick,
        reason = "the asks allocate only on ticks that ask for columns, and the view list only when the views change, as it moves into the generation link"
    )
)]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(2))]
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
    for ask in cells.take_generation_asks() {
        if let Some(stage) = generation.worlds.get_mut(&ask.label) {
            stage.ask(ask.cell, ask.column);
        }
    }
    for stage in generation.worlds.values_mut() {
        stage.send_views();
    }
}

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

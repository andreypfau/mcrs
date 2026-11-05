use std::collections::BTreeMap;
use std::ops::Range;

use bevy_ecs::entity::Entity;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::{Local, ResMut};
use bevy_ecs::world::World;
use mcrs_engine_cell::CellId;
use mcrs_engine_link::{Link, Placement};
use mcrs_minecraft_engine::column_generation::GenerationAnswer;
use mcrs_minecraft_engine::coordinator::dim_cells::DimCells;
use mcrs_minecraft_engine::player_view::PlayerView;
use mcrs_minecraft_engine::stored_column::{LitBy, StoredColumn, StoredSection};
use mcrs_minecraft_engine_world::aoi::min_column_distance;
use mcrs_minecraft_engine_world::entity::player::chunk_view::ChunkTrackingView;
use mcrs_minecraft_engine_world::world::dimension_builders::DimensionBuild;
use mcrs_minecraft_worldgen_generator::stages::FillContext;
use mcrs_voxel::{ColumnPos, SectionPos};
use rustc_hash::FxHashMap;
use tracing::warn;

use crate::generation_input::{GenerationInput, ViewUpdate, WantedColumn};
use crate::generation_output::{GeneratedColumn, GenerationOutput, SectionContent};
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
    rows: Range<i32>,
    /// The cells that wait on each column the stage generates.
    askers: FxHashMap<ColumnPos, Vec<CellId>>,
    /// The view of each player of the world, by session.
    views: BTreeMap<u64, ChunkTrackingView>,
    views_changed: Changed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Changed {
    No,
    Yes,
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
    fn note_view(&mut self, view: PlayerView) {
        let changed = match view.view {
            Some(tracked) => self.views.insert(view.session.0, tracked) != Some(tracked),
            None => self.views.remove(&view.session.0).is_some(),
        };
        if changed {
            self.views_changed = Changed::Yes;
        }
    }

    fn ask(&mut self, cell: CellId, column: ColumnPos, players: &[ColumnPos]) {
        let askers = self.askers.entry(column).or_default();
        if askers.is_empty() {
            self.link.send(GenerationInput::Want(WantedColumn {
                col: column,
                distance_sq: min_column_distance(&column, players),
                rows: self.rows.clone().collect(),
            }));
        }
        if !askers.contains(&cell) {
            askers.push(cell);
        }
    }

    fn players(&self) -> Vec<ColumnPos> {
        self.views
            .values()
            .map(|view| ColumnPos::new(view.center.x, view.center.z))
            .collect()
    }

    fn send_views(&mut self, players: Vec<ColumnPos>) {
        if self.views_changed == Changed::No {
            return;
        }
        self.views_changed = Changed::No;
        self.link.send(GenerationInput::Views(ViewUpdate {
            views: self.views.values().copied().collect(),
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
    let mut link = Link::new(GenerationStage::new(config), Placement::Thread);
    link.send(GenerationInput::Context(Box::new(fill.clone())));
    host.get_resource_or_init::<WorldGeneration>()
        .worlds
        .insert(
            label,
            WorldStage {
                link,
                rows: first..first.saturating_add(count),
                askers: FxHashMap::default(),
                views: BTreeMap::new(),
                views_changed: Changed::No,
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
            stage.note_view(reported.view);
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
        finished.extend(stage.link.outputs().map(|output| match output {
            GenerationOutput::Generated(column) => GenerationAnswer {
                column: column.col,
                stored: stored_column(*column),
            },
            GenerationOutput::Abandoned(column) => GenerationAnswer {
                column,
                stored: None,
            },
        }));
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

/// The column as the data layer keeps it, unlit, or `None` when a section has no blocks.
fn stored_column(column: GeneratedColumn) -> Option<StoredColumn> {
    let min_section_y = column.sections.first()?.y;
    let mut sections = Vec::with_capacity(column.sections.len());
    for section in column.sections {
        let SectionContent::Blocks(content) = section.content else {
            warn!(
                x = column.col.x,
                z = column.col.z,
                y = section.y,
                "a generated column lacks the blocks of a section"
            );
            return None;
        };
        sections.push(StoredSection {
            blocks: content.blocks,
            biomes: content.biomes,
            block_light: None,
            sky_light: None,
        });
    }
    Some(StoredColumn {
        min_section_y,
        sections,
        block_entities: column.block_entities,
        heightmaps: column.maps,
        lit_by: LitBy::Unlit,
        ticks: column.ticks,
    })
}

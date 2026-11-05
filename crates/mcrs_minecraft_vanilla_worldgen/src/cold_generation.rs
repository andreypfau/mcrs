use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::prelude::{Local, Query, With, resource_exists_and_changed};
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_ecs::system::{Res, ResMut};
use mcrs_engine_link::{Link, Placement};
use mcrs_minecraft_engine::column_production::{ColdStageLandSet, ColumnProduction};
use mcrs_minecraft_engine::column_streams::ColumnStreamSet;
use mcrs_minecraft_engine::stored_column::{LitBy, StoredColumn, StoredSection};
use mcrs_minecraft_engine_world::aoi::min_column_distance;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::tick::TickSet;
use mcrs_minecraft_engine_world::world::dimension::DimensionTypeConfig;
use mcrs_minecraft_worldgen_generator::stages::FillContext;
use mcrs_voxel::{ColumnPos, SectionPos};
use tracing::warn;

use crate::chunk_plugin::send_fill_context;
use crate::generation_input::{GenerationInput, RequestedSection, ViewUpdate, WantedColumn};
use crate::generation_link::GenerationLink;
use crate::generation_output::{GeneratedColumn, GenerationOutput, SectionContent};
use crate::generation_stage::{GenerationStage, SchedulerConfig};

/// The generation stage of cold space: a second generation stage that generates whole columns
/// for the data layer of the dimension, tagged by section row. It owns no section entity, so
/// what the dimension simulates does not depend on when it is done.
pub(crate) struct ColdGenerationPlugin {
    pub(crate) placement: Placement,
}

impl Plugin for ColdGenerationPlugin {
    fn build(&self, app: &mut App) {
        let link: Link<GenerationStage<i32>> = Link::new(
            GenerationStage::new(None, SchedulerConfig::default()),
            self.placement,
        );
        app.insert_resource(GenerationLink::new(link));
        app.add_systems(
            FixedUpdate,
            (
                land_cold_columns.in_set(ColdStageLandSet::Generation),
                (
                    send_fill_context::<i32>.run_if(resource_exists_and_changed::<FillContext>),
                    request_cold_columns,
                    send_cold_views,
                )
                    .chain()
                    .in_set(TickSet::Project)
                    .after(ColumnStreamSet),
            ),
        );
    }
}

/// Hands the generation stage of cold space the columns that the data layer generates, each
/// with every section row of the dimension.
fn request_cold_columns(
    mut production: ResMut<ColumnProduction>,
    mut generation: ResMut<GenerationLink<i32>>,
    dims: Query<&DimensionTypeConfig>,
    players: Query<&Transform, With<Player>>,
    mut player_positions: Local<Vec<ColumnPos>>,
) {
    let Some(rows) = dims.iter().next() else {
        return;
    };
    let first = rows.min_y >> SectionPos::BITS;
    let count = i32::try_from(rows.section_count).unwrap_or(0);
    player_positions.clear();
    player_positions.extend(players.iter().map(|at| ColumnPos::from(at.translation)));
    for col in production.take_generation() {
        generation.send(GenerationInput::Want(WantedColumn {
            col,
            distance_sq: min_column_distance(&col, &player_positions),
            sections: (first..first.saturating_add(count))
                .map(|y| RequestedSection { tag: y, y })
                .collect(),
        }));
    }
}

/// Tells the generation stage of cold space the views that the data layer serves, which hold
/// the views of the players of the other cells of the world, so it drops what left all of them.
fn send_cold_views(
    production: Res<ColumnProduction>,
    mut generation: ResMut<GenerationLink<i32>>,
    players: Query<&Transform, With<Player>>,
) {
    generation.send(GenerationInput::Views(ViewUpdate {
        views: production.views().to_vec(),
        players: players
            .iter()
            .map(|transform| ColumnPos::from(transform.translation))
            .collect(),
    }));
}

/// Hands the data layer what the generation stage of cold space generated or dropped.
fn land_cold_columns(
    mut production: ResMut<ColumnProduction>,
    mut generation: ResMut<GenerationLink<i32>>,
) {
    for output in generation.outputs() {
        match output {
            GenerationOutput::Generated(column) => {
                let col = column.col;
                production.generated(col, stored_column(*column));
            }
            GenerationOutput::Abandoned(column) => production.generated(column.col, None),
        }
    }
}

/// The column as the data layer keeps it, unlit, or `None` when a section has no blocks.
fn stored_column(column: GeneratedColumn<i32>) -> Option<StoredColumn> {
    let min_section_y = column.sections.first()?.tag;
    let mut sections = Vec::with_capacity(column.sections.len());
    for section in column.sections {
        let SectionContent::Blocks(content) = section.content else {
            warn!(
                x = column.col.x,
                z = column.col.z,
                y = section.tag,
                "a column generated for the data layer lacks the blocks of a section"
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

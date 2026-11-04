use bevy_ecs::entity::Entity;
use bevy_ecs::message::MessageReader;
use bevy_ecs::prelude::{Commands, Component, Local, ParamSet, Query, ResMut};
use mcrs_engine_store::{ChunkVersion, StoreOutput};
use mcrs_minecraft_engine_world::world::dimension::InDimension;
use mcrs_minecraft_engine_world::world::lifecycle::stage::{
    SectionStage, SectionStageChanged, SectionStages,
};
use mcrs_minecraft_engine_world::world::storage::block_entity::spawn_block_entities;
use mcrs_voxel::{ColumnPos, SectionPos};

use crate::column_store::{ColumnStoreLink, Viewed, ViewedColumn};
use crate::heightmap::PendingColumnHeightmaps;
use crate::stored_column::StoredColumn;

/// The section came back from the data layer, so the data layer already holds its blocks.
#[derive(Component, Clone, Copy, Debug, Default)]
#[component(storage = "SparseSet")]
pub struct TakenFromStore;

/// Sends the data layer a request for each column whose sections a ticket loads, before they
/// are generated. A section that the data layer did not hold comes back as `Loading` and goes
/// on to generation.
pub(crate) fn fetch_loading_columns(
    mut stages: ParamSet<(MessageReader<SectionStageChanged>, SectionStages)>,
    mut loading: Local<Vec<SectionStageChanged>>,
    mut store: ResMut<ColumnStoreLink>,
) {
    loading.extend(stages.p0().read().filter(|change| {
        change.to == SectionStage::Loading && change.from != Some(SectionStage::Fetching)
    }));
    let mut stages = stages.p1();
    for change in loading.drain(..) {
        let column = ColumnPos::from(change.pos);
        if !store.may_hold(column) || stages.get(change.section) != Some(SectionStage::Loading) {
            continue;
        }
        stages.set(change.section, SectionStage::Fetching);
        store.take(change.dim, column, change.section);
    }
}

/// Lands each answer of the data layer: in the sections that wait to take the column back, and
/// in the list of answers that views wait on.
pub(crate) fn land_stored_columns(
    mut store: ResMut<ColumnStoreLink>,
    mut stages: SectionStages,
    positions: Query<&SectionPos>,
    mut pending_heightmaps: ResMut<PendingColumnHeightmaps>,
    mut commands: Commands,
    mut answers: Local<Vec<StoreOutput<ColumnPos, StoredColumn>>>,
) {
    answers.extend(store.outputs());
    for answer in answers.drain(..) {
        let (column, version, stored) = match answer {
            StoreOutput::Found { key, chunk } => (key, chunk.version, Some(chunk.chunk)),
            // The data layer lives in memory only, so it never fails to read.
            StoreOutput::Missing { key } | StoreOutput::Unreadable { key } => {
                (key, ChunkVersion::default(), None)
            }
        };
        if store
            .reads
            .get(&column)
            .is_none_or(|read| version < read.at_least)
        {
            continue;
        }
        let Some(read) = store.reads.remove(&column) else {
            continue;
        };
        match &stored {
            Some(stored) => {
                land_sections(
                    &mut stages,
                    &positions,
                    &mut commands,
                    read.dim,
                    &read.sections,
                    stored,
                );
                if let Some(maps) = &stored.heightmaps
                    && !read.sections.is_empty()
                {
                    pending_heightmaps
                        .0
                        .insert((read.dim, column), maps.clone());
                }
            }
            None => {
                for &section in &read.sections {
                    if stages.get(section) == Some(SectionStage::Fetching) {
                        stages.set(section, SectionStage::Loading);
                    }
                }
            }
        }
        if read.viewed == Viewed::Yes {
            store.viewed.push(ViewedColumn {
                dim: read.dim,
                column,
                stored,
            });
        }
    }
}

fn land_sections(
    stages: &mut SectionStages,
    positions: &Query<&SectionPos>,
    commands: &mut Commands,
    dim: Entity,
    sections: &[Entity],
    stored: &StoredColumn,
) {
    let mut landed_rows: Vec<i32> = Vec::new();
    for &section in sections {
        if stages.get(section) != Some(SectionStage::Fetching) {
            continue;
        }
        let Some(y) = positions.get(section).ok().map(|pos| pos.y) else {
            continue;
        };
        let Some(content) = stored.section(y) else {
            stages.set(section, SectionStage::Loading);
            continue;
        };
        let mut entity = commands.entity(section);
        entity.try_insert((
            content.blocks.clone(),
            content.biomes.clone(),
            TakenFromStore,
        ));
        if let Some(light) = &content.block_light {
            entity.try_insert(light.clone());
        }
        if let Some(light) = &content.sky_light {
            entity.try_insert(light.clone());
        }
        stages.set(section, SectionStage::Loaded);
        landed_rows.push(y);
    }
    let block_entities = stored
        .block_entities
        .iter()
        .filter(|entry| landed_rows.contains(&(entry.position().y >> SectionPos::BITS)))
        .cloned()
        .collect();
    spawn_block_entities(commands, InDimension(dim), block_entities);
}

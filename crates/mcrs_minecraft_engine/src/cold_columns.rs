use bevy_ecs::message::MessageReader;
use bevy_ecs::prelude::{Commands, Component, Has, Query, Res, ResMut};
use mcrs_minecraft_chunk::{BlockLight, SkyLight};
use mcrs_minecraft_engine_world::block_update::BlockPlaced;
use mcrs_minecraft_engine_world::palette::{BiomePalette, ChunkBlocks};
use mcrs_minecraft_engine_world::world::lifecycle::column_loading::ColumnLoading;
use mcrs_minecraft_engine_world::world::lifecycle::stage::SectionStage;
use mcrs_minecraft_engine_world::world::light_gate::{ColumnLightSettled, LightGated};
use mcrs_minecraft_engine_world::world::storage::block_entity::{
    BlockEntity, SectionBlockEntities,
};
use mcrs_minecraft_engine_world::world::storage::column::{
    ColumnIndex, ColumnSections, SectionLookup,
};
use mcrs_minecraft_heightmap::{
    ColumnHeightmapSet, MotionHeightmap, NoLeavesHeightmap, SolidHeightmap, SurfaceHeightmap,
};

use crate::column_store::ColumnStoreLink;
use crate::stored_column::{StoredColumn, StoredSection};
use crate::stored_columns_landing::TakenFromStore;

/// The light of the column, and of the eight around it, settled while it was loaded, so the
/// light it holds is the light it keeps when it goes cold.
#[derive(Component, Clone, Copy, Debug, Default)]
#[component(storage = "SparseSet")]
pub struct ColumnLightFinal;

/// A block of the section changed since it was generated or taken from the data layer.
#[derive(Component, Clone, Copy, Debug, Default)]
#[component(storage = "SparseSet")]
pub struct SectionChanged;

pub(crate) fn mark_final_light(
    mut settled: MessageReader<ColumnLightSettled>,
    indices: Query<&ColumnIndex>,
    mut commands: Commands,
) {
    for &ColumnLightSettled(column) in settled.read() {
        for index in &indices {
            if let Some(slot) = index.get(&column) {
                commands.entity(slot.entity).try_insert(ColumnLightFinal);
            }
        }
    }
}

pub(crate) fn mark_changed_sections(
    mut placed: MessageReader<BlockPlaced>,
    changed: Query<(), bevy_ecs::prelude::Without<SectionChanged>>,
    mut commands: Commands,
) {
    for placed in placed.read() {
        if changed.contains(placed.chunk) {
            commands.entity(placed.chunk).try_insert(SectionChanged);
        }
    }
}

type SectionMarks = (
    &'static SectionStage,
    Has<TakenFromStore>,
    Has<SectionChanged>,
);

type SectionContent = (
    &'static ChunkBlocks,
    &'static BiomePalette,
    Option<&'static BlockLight>,
    Option<&'static SkyLight>,
    Option<&'static SectionBlockEntities>,
);

type ColumnContent = (
    &'static ColumnSections,
    Has<ColumnLightFinal>,
    Option<(
        &'static SurfaceHeightmap,
        &'static SolidHeightmap,
        &'static MotionHeightmap,
        &'static NoLeavesHeightmap,
    )>,
);

/// Where the content of a column came from since it was loaded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Origin {
    /// Every section came back from the data layer, and none changed.
    Unchanged,
    /// Some section came back from the data layer, or some section changed.
    Kept,
    /// Every section was generated, and none changed.
    Generated,
}

/// Writes the last version of each column that stopped being loaded, while its sections still
/// hold their blocks.
///
/// A column that never landed whole keeps the version the data layer already holds, and so
/// does a column that the data layer handed back and that did not change. A generated column
/// whose light never settled is not written: its light at the edge of the loaded space was
/// lit without the neighbours, and a view would get it that way.
pub(crate) fn write_cold_columns(
    dims: Query<(&ColumnLoading, &ColumnIndex)>,
    columns: Query<ColumnContent>,
    marks: Query<SectionMarks>,
    sections: Query<SectionContent>,
    block_entities: Query<&BlockEntity>,
    light_gated: Option<Res<LightGated>>,
    mut store: ResMut<ColumnStoreLink>,
) {
    for (loading, index) in &dims {
        for &column in loading.cooling() {
            if loading.is_column_loaded(column) {
                continue;
            }
            let Some(slot) = index.get(&column) else {
                continue;
            };
            let Ok((column_sections, light_final, heightmaps)) = columns.get(slot.entity) else {
                continue;
            };
            let Some(origin) = origin(column_sections, &marks) else {
                continue;
            };
            let unlit = light_gated.is_some() && !light_final;
            match origin {
                Origin::Unchanged => continue,
                Origin::Generated if unlit => continue,
                Origin::Generated | Origin::Kept => {}
            }
            let Some(mut stored) = last_version(column_sections, &sections, &block_entities) else {
                continue;
            };
            stored.heightmaps =
                heightmaps.map(|(surface, solid, motion, no_leaves)| ColumnHeightmapSet {
                    surface: surface.clone(),
                    solid: solid.clone(),
                    motion: motion.clone(),
                    no_leaves: no_leaves.clone(),
                });
            store.put(column, stored);
        }
    }
}

/// Where the content of a column came from, or `None` when a section has not landed.
fn origin(column: &ColumnSections, marks: &Query<SectionMarks>) -> Option<Origin> {
    let mut any_taken = false;
    let mut all_taken = true;
    let mut any_changed = false;
    for lookup in column.iter() {
        let SectionLookup::Loaded(section) = lookup else {
            return None;
        };
        let (stage, taken, changed) = marks.get(section).ok()?;
        if *stage != SectionStage::Loaded {
            return None;
        }
        any_taken |= taken;
        all_taken &= taken;
        any_changed |= changed;
    }
    Some(if all_taken && !any_changed {
        Origin::Unchanged
    } else if any_taken || any_changed {
        Origin::Kept
    } else {
        Origin::Generated
    })
}

/// The sections and block entities of a column, or `None` when a section has not landed.
fn last_version(
    column: &ColumnSections,
    sections: &Query<SectionContent>,
    block_entities: &Query<&BlockEntity>,
) -> Option<StoredColumn> {
    let mut stored = StoredColumn {
        min_section_y: column.min_section_y,
        sections: Vec::with_capacity(column.sections.len()),
        block_entities: Vec::new(),
        heightmaps: None,
    };
    for lookup in column.iter() {
        let SectionLookup::Loaded(section) = lookup else {
            return None;
        };
        let (blocks, biomes, block_light, sky_light, held) = sections.get(section).ok()?;
        stored.sections.push(StoredSection {
            blocks: blocks.clone(),
            biomes: biomes.clone(),
            block_light: block_light.cloned(),
            sky_light: sky_light.cloned(),
        });
        stored.block_entities.extend(
            held.into_iter()
                .flat_map(|held| held.iter())
                .filter_map(|entity| block_entities.get(*entity).ok())
                .map(|BlockEntity(entry)| entry.clone()),
        );
    }
    Some(stored)
}

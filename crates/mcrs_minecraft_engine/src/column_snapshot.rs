use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::Query;
use bevy_ecs::system::SystemParam;
use mcrs_minecraft_chunk::{BlockLight, SkyLight};
use mcrs_minecraft_engine_world::palette::{BiomePalette, ChunkBlocks};
use mcrs_minecraft_engine_world::world::lifecycle::stage::SectionStage;
use mcrs_minecraft_engine_world::world::storage::block_entity::{
    BlockEntity, SectionBlockEntities,
};
use mcrs_minecraft_engine_world::world::storage::column::{ColumnSections, SectionLookup};
use mcrs_minecraft_heightmap::{
    ColumnHeightmapSet, MotionHeightmap, NoLeavesHeightmap, SolidHeightmap, SurfaceHeightmap,
};

use crate::stored_column::{LitBy, StoredColumn, StoredSection};

type ColumnContent = (
    &'static ColumnSections,
    Option<(
        &'static SurfaceHeightmap,
        &'static SolidHeightmap,
        &'static MotionHeightmap,
        &'static NoLeavesHeightmap,
    )>,
);

type SectionContent = (
    &'static SectionStage,
    &'static ChunkBlocks,
    &'static BiomePalette,
    Option<&'static BlockLight>,
    Option<&'static SkyLight>,
    Option<&'static SectionBlockEntities>,
);

/// The content of the loaded columns of a dimension, as the data layer keeps a column.
#[derive(SystemParam)]
pub(crate) struct ColumnSnapshots<'w, 's> {
    columns: Query<'w, 's, ColumnContent>,
    sections: Query<'w, 's, SectionContent>,
    block_entities: Query<'w, 's, &'static BlockEntity>,
}

impl ColumnSnapshots<'_, '_> {
    /// The sections, block entities, heightmaps and light of the column entity `column` now,
    /// or `None` while a section has not landed.
    pub(crate) fn take(&self, column: Entity) -> Option<StoredColumn> {
        let (column_sections, heightmaps) = self.columns.get(column).ok()?;
        let mut stored = StoredColumn {
            min_section_y: column_sections.min_section_y,
            sections: Vec::with_capacity(column_sections.sections.len()),
            block_entities: Vec::new(),
            heightmaps: heightmaps.map(|(surface, solid, motion, no_leaves)| ColumnHeightmapSet {
                surface: surface.clone(),
                solid: solid.clone(),
                motion: motion.clone(),
                no_leaves: no_leaves.clone(),
            }),
            lit_by: LitBy::Dimension,
        };
        for lookup in column_sections.iter() {
            let SectionLookup::Loaded(section) = lookup else {
                return None;
            };
            let (stage, blocks, biomes, block_light, sky_light, held) =
                self.sections.get(section).ok()?;
            if *stage != SectionStage::Loaded {
                return None;
            }
            stored.sections.push(StoredSection {
                blocks: blocks.clone(),
                biomes: biomes.clone(),
                block_light: block_light.cloned(),
                sky_light: sky_light.cloned(),
            });
            stored.block_entities.extend(
                held.into_iter()
                    .flat_map(|held| held.iter())
                    .filter_map(|entity| self.block_entities.get(*entity).ok())
                    .map(|BlockEntity(entry)| entry.clone()),
            );
        }
        Some(stored)
    }
}

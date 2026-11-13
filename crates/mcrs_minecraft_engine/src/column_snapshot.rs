use std::ops::Range;

use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::Query;
use bevy_ecs::system::SystemParam;
use mcrs_minecraft_chunk::{BlockLight, SkyLight};
use mcrs_minecraft_engine_world::palette::BiomePalette;
use mcrs_minecraft_engine_world::world::dimension::InDimension;
use mcrs_minecraft_engine_world::world::lifecycle::stage::SectionStage;
use mcrs_minecraft_engine_world::world::storage::block_entity::{
    BlockEntity, SectionBlockEntities,
};
use mcrs_minecraft_engine_world::world::storage::column::{ColumnSections, SectionLookup};
use mcrs_minecraft_engine_world::world::storage::section::SectionIndex;
use mcrs_minecraft_heightmap::{
    ColumnHeightmapSet, MotionHeightmap, NoLeavesHeightmap, SolidHeightmap, SurfaceHeightmap,
};
use mcrs_voxel::SectionPos;

use crate::stored_column::{ColumnRecord, ColumnStructures, RawBlockEntities};
use mcrs_minecraft_region_disk::column_ticks::ColumnTicks;
use mcrs_minecraft_region_disk::stored_column::{StoredColumn, StoredSection};

type ColumnContent = (
    &'static ColumnSections,
    Option<(
        &'static SurfaceHeightmap,
        &'static SolidHeightmap,
        &'static MotionHeightmap,
        &'static NoLeavesHeightmap,
    )>,
    Option<&'static ColumnStructures>,
    Option<&'static ColumnRecord>,
);

type SectionContent = (
    &'static SectionStage,
    &'static SectionPos,
    &'static InDimension,
    &'static BiomePalette,
    Option<&'static BlockLight>,
    Option<&'static SkyLight>,
    Option<&'static SectionBlockEntities>,
    Option<&'static RawBlockEntities>,
);

/// The content of the loaded columns of a dimension, as the data layer keeps a column.
#[derive(SystemParam)]
pub(crate) struct ColumnSnapshots<'w, 's> {
    columns: Query<'w, 's, ColumnContent>,
    sections: Query<'w, 's, SectionContent>,
    section_indices: Query<'w, 's, &'static SectionIndex>,
    block_entities: Query<'w, 's, &'static BlockEntity>,
}

/// Whether a snapshot holds the light of the dimension.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Light {
    Keep,
    Drop,
}

impl ColumnSnapshots<'_, '_> {
    /// The sections, block entities, heightmaps and light of the column entity `column` now,
    /// or `None` while a section has not landed.
    pub(crate) fn take(&self, column: Entity) -> Option<StoredColumn> {
        self.snapshot(column, Light::Keep)
    }

    /// The section rows of the column entity `column`.
    pub(crate) fn rows(&self, column: Entity) -> Option<Range<i32>> {
        let (column_sections, ..) = self.columns.get(column).ok()?;
        let min_y = column_sections.min_section_y;
        let end = i32::try_from(column_sections.sections.len())
            .ok()?
            .checked_add(min_y)?;
        Some(min_y..end)
    }

    /// The snapshot of [`Self::take`] without the light, which the data layer computes again
    /// from the blocks.
    pub(crate) fn take_unlit(&self, column: Entity) -> Option<StoredColumn> {
        self.snapshot(column, Light::Drop)
    }

    fn snapshot(&self, column: Entity, light: Light) -> Option<StoredColumn> {
        let (column_sections, heightmaps, structures, record) = self.columns.get(column).ok()?;
        let (surface, solid, motion, no_leaves) = heightmaps?;
        let mut stored = StoredColumn {
            status: mcrs_minecraft_anvil::ChunkStatus::Full,
            is_light_on: false,
            inhabited_time: record.map_or(0, |record| record.inhabited_time),
            last_update: record.map_or(0, |record| record.last_update),
            min_section_y: column_sections.min_section_y,
            sections: Vec::with_capacity(column_sections.sections.len()),
            block_entities: Vec::new(),
            unknown_block_entities: Vec::new(),
            retrogen: record.and_then(|record| record.retrogen.clone()),
            post_processing: record
                .map(|record| record.post_processing.clone())
                .unwrap_or_default(),
            upgrade_data: record
                .map(|record| record.upgrade_data.clone())
                .unwrap_or_default(),
            blending_data: record.and_then(|record| record.blending_data.clone()),
            structures: structures
                .map(|structures| structures.0.clone())
                .unwrap_or_default(),
            heightmaps: ColumnHeightmapSet {
                surface: surface.clone(),
                solid: solid.clone(),
                motion: motion.clone(),
                no_leaves: no_leaves.clone(),
            },
            ticks: ColumnTicks::default(),
        };
        for lookup in column_sections.iter() {
            let SectionLookup::Loaded(section) = lookup else {
                return None;
            };
            let (stage, &pos, &InDimension(dim), biomes, block_light, sky_light, held, raw) =
                self.sections.get(section).ok()?;
            if *stage != SectionStage::Loaded {
                return None;
            }
            let blocks = self.section_indices.get(dim).ok()?.blocks(pos)?;
            stored.sections.push(StoredSection {
                blocks: blocks.clone(),
                biomes: biomes.clone(),
                block_light: block_light.filter(|_| light == Light::Keep).cloned(),
                sky_light: sky_light.filter(|_| light == Light::Keep).cloned(),
            });
            stored.block_entities.extend(
                held.into_iter()
                    .flat_map(|held| held.iter())
                    .filter_map(|entity| self.block_entities.get(*entity).ok())
                    .map(|BlockEntity(entry)| entry.clone()),
            );
            if let Some(raw) = raw {
                stored.unknown_block_entities.extend(raw.0.iter().cloned());
            }
        }
        Some(stored)
    }
}

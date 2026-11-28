pub use mcrs_voxel::SectionPos;

use crate::palette::ChunkBlocks;
use crate::world::dimension::InDimension;
use crate::world::storage::random_ticks::RandomTickIndex;
use crate::world::storage::section_stage::SectionStage;
use bevy_ecs::prelude::{Bundle, Component, Entity};
use mcrs_minecraft_simulation::TickingSectionMut;
use mcrs_minecraft_volume::{PalettedSection, PalettedSectionMut};
use rustc_hash::FxHashMap;

#[derive(Bundle)]
pub struct SectionBundle {
    pub dimension: InDimension,
    pub pos: SectionPos,
    marker: Section,
    stage: SectionStage,
}

#[derive(Component, Debug, Default)]
#[component(storage = "SparseSet")]
pub struct Section;

impl SectionBundle {
    pub fn new(dimension: InDimension, chunk_pos: SectionPos) -> Self {
        Self {
            dimension,
            pos: chunk_pos,
            marker: Section,
            stage: SectionStage::Loading,
        }
    }
}

#[derive(Component, Debug, Default)]
pub struct SectionIndex(FxHashMap<SectionPos, IndexEntry>);

#[derive(Debug)]
struct IndexEntry {
    entity: Entity,
    blocks: Option<ChunkBlocks>,
}

impl SectionIndex {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, pos: impl Into<SectionPos>) -> Option<Entity> {
        self.0.get(&pos.into()).map(|section| section.entity)
    }

    pub fn insert(&mut self, pos: SectionPos, entity: Entity) {
        self.0.insert(
            pos,
            IndexEntry {
                entity,
                blocks: None,
            },
        );
    }

    pub fn remove(
        &mut self,
        pos: SectionPos,
        random_ticks: Option<&mut RandomTickIndex>,
    ) -> Option<Entity> {
        if let Some(random_ticks) = random_ticks {
            random_ticks.remove(pos);
        }
        self.0.remove(&pos).map(|section| section.entity)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn blocks(&self, pos: SectionPos) -> Option<&ChunkBlocks> {
        self.0.get(&pos)?.blocks.as_ref()
    }

    pub fn section_mut<'a>(
        &'a mut self,
        pos: SectionPos,
        random_ticks: Option<&'a mut RandomTickIndex>,
    ) -> Option<TickingSectionMut<'a, PalettedSectionMut<'a>>> {
        let blocks = self.0.get_mut(&pos)?.blocks.as_mut()?;
        let blocks = PalettedSectionMut(&mut blocks.make_mut().0);
        Some(match random_ticks {
            Some(index) => index.section_mut(pos, blocks),
            None => TickingSectionMut::untracked(blocks),
        })
    }

    pub fn land(
        &mut self,
        pos: SectionPos,
        entity: Entity,
        blocks: ChunkBlocks,
        random_ticks: Option<&mut RandomTickIndex>,
        random_ticked: bool,
    ) -> bool {
        let Some(section) = self
            .0
            .get_mut(&pos)
            .filter(|section| section.entity == entity)
        else {
            return false;
        };
        let blocks = section.blocks.insert(blocks);
        if let Some(random_ticks) = random_ticks {
            random_ticks.insert(pos, &PalettedSection(&blocks.0.0), random_ticked);
        }
        true
    }

    pub fn loaded(&self) -> impl Iterator<Item = (SectionPos, Entity, &ChunkBlocks)> {
        self.0
            .iter()
            .filter_map(|(pos, section)| Some((*pos, section.entity, section.blocks.as_ref()?)))
    }
}

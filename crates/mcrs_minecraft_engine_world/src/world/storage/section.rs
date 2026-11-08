pub use mcrs_voxel::SectionPos;

use crate::entity::SectionEntities;
use crate::palette::ChunkBlocks;
use crate::world::dimension::InDimension;
use crate::world::lifecycle::stage::SectionStage;
use crate::world::lifecycle::ticket::TicketPlugin;
use bevy_app::{App, Plugin};
use bevy_ecs::prelude::{Bundle, Component, Entity};
use rustc_hash::FxHashMap;

pub(crate) struct SectionPlugin;

impl Plugin for SectionPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(TicketPlugin);
    }
}

#[derive(Bundle)]
pub struct SectionBundle {
    pub dimension: InDimension,
    pub pos: SectionPos,
    pub entities: SectionEntities,
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
            entities: SectionEntities::default(),
            marker: Section,
            stage: SectionStage::Loading,
        }
    }
}

/// The sections of a dimension by position: the entity of each, and the blocks of each loaded
/// one. The index is the only owner of the blocks, so a read of a block finds them with one
/// lookup, without the entity.
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

    /// Indexes a section with no blocks yet, in place of what the index held at `pos`.
    pub fn insert(&mut self, pos: SectionPos, entity: Entity) {
        self.0.insert(
            pos,
            IndexEntry {
                entity,
                blocks: None,
            },
        );
    }

    /// Forgets the section at `pos` with its blocks.
    pub fn remove(&mut self, pos: impl Into<SectionPos>) -> Option<Entity> {
        self.0.remove(&pos.into()).map(|section| section.entity)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The blocks of the section at `pos`, once they landed.
    pub fn blocks(&self, pos: SectionPos) -> Option<&ChunkBlocks> {
        self.0.get(&pos)?.blocks.as_ref()
    }

    pub fn blocks_mut(&mut self, pos: SectionPos) -> Option<&mut ChunkBlocks> {
        self.0.get_mut(&pos)?.blocks.as_mut()
    }

    /// Gives `entity`, the section at `pos`, its blocks. `None` when the index holds another
    /// section there, or none.
    pub fn land(
        &mut self,
        pos: SectionPos,
        entity: Entity,
        blocks: ChunkBlocks,
    ) -> Option<&ChunkBlocks> {
        let section = self
            .0
            .get_mut(&pos)
            .filter(|section| section.entity == entity)?;
        Some(section.blocks.insert(blocks))
    }

    /// The loaded sections with their blocks, in no order.
    pub fn loaded(&self) -> impl Iterator<Item = (SectionPos, Entity, &ChunkBlocks)> {
        self.0
            .iter()
            .filter_map(|(pos, section)| Some((*pos, section.entity, section.blocks.as_ref()?)))
    }
}

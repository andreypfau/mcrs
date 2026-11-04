use bevy_ecs::entity::Entity;
use mcrs_minecraft_block_entity::GeneratedBlockEntity;
use mcrs_minecraft_engine_world::palette::{BiomePalette, ChunkBlocks};
use mcrs_minecraft_heightmap::ColumnHeightmapSet;
use mcrs_minecraft_simulation::PendingTick;
use mcrs_minecraft_worldgen_feature::placer::GenerationKinds;
use mcrs_minecraft_worldgen_feature_place::entity::GeneratedEntity;
use mcrs_minecraft_worldgen_generator::task::ColumnSource;
use mcrs_voxel::ColumnPos;

/// What the generation stage sends back to the dimension.
pub(crate) enum GenerationOutput {
    Generated(Box<GeneratedColumn>),
    /// A column that left every view before it reached the top of its ladder.
    Abandoned(AbandonedColumn),
}

/// A column at the top of its ladder, already in the form its sections take in the ECS.
pub(crate) struct GeneratedColumn {
    pub(crate) col: ColumnPos,
    pub(crate) source: ColumnSource,
    pub(crate) maps: Option<ColumnHeightmapSet>,
    pub(crate) block_entities: Vec<GeneratedBlockEntity>,
    pub(crate) entities: Vec<GeneratedEntity>,
    /// The sections the requests asked for, bottom first.
    pub(crate) sections: Vec<GeneratedSection>,
}

pub(crate) struct GeneratedSection {
    pub(crate) entity: Entity,
    pub(crate) content: SectionContent,
}

pub(crate) enum SectionContent {
    Blocks(SectionBlocks),
    /// The section has blocks, but no block table to check them against.
    Unchecked,
    Missing,
}

pub(crate) struct SectionBlocks {
    pub(crate) blocks: ChunkBlocks,
    pub(crate) biomes: BiomePalette,
    /// The ticks generation scheduled in the section, in the order it asked.
    pub(crate) ticks: Vec<PendingTick<GenerationKinds>>,
}

pub(crate) struct AbandonedColumn {
    pub(crate) col: ColumnPos,
    /// The sections that asked for the column.
    pub(crate) sections: Vec<Entity>,
}

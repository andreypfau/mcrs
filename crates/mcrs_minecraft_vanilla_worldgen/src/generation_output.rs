use mcrs_minecraft_block_entity::GeneratedBlockEntity;
use mcrs_minecraft_engine::column_ticks::ColumnTicks;
use mcrs_minecraft_engine_world::palette::{BiomePalette, ChunkBlocks};
use mcrs_minecraft_heightmap::ColumnHeightmapSet;
use mcrs_voxel::ColumnPos;

/// What the generation stage sends back to the dimension.
pub(crate) enum GenerationOutput<T> {
    Generated(Box<GeneratedColumn<T>>),
    /// A column that left every view before it reached the top of its ladder.
    Abandoned(ColumnPos),
}

/// A column at the top of its ladder, already in the form its sections take in the ECS.
pub(crate) struct GeneratedColumn<T> {
    pub(crate) col: ColumnPos,
    pub(crate) maps: Option<ColumnHeightmapSet>,
    pub(crate) block_entities: Vec<GeneratedBlockEntity>,
    /// The sections the requests asked for, bottom first.
    pub(crate) sections: Vec<GeneratedSection<T>>,
    /// The ticks that generation scheduled in the column, in the order it asked.
    pub(crate) ticks: ColumnTicks,
}

pub(crate) struct GeneratedSection<T> {
    pub(crate) tag: T,
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
}

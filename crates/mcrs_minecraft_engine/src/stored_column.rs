use mcrs_minecraft_block_entity::GeneratedBlockEntity;
use mcrs_minecraft_chunk::{BlockLight, SkyLight};
use mcrs_minecraft_engine_world::palette::{BiomePalette, ChunkBlocks};
use mcrs_minecraft_heightmap::ColumnHeightmapSet;

/// The last version of a column, as the data layer keeps it. A client receives whole columns,
/// so Minecraft keys the data layer by column and not by section.
#[derive(Clone, Debug)]
pub struct StoredColumn {
    /// The Y of the first entry of `sections`.
    pub min_section_y: i32,
    /// Every section of the column, in ascending Y.
    pub sections: Vec<StoredSection>,
    pub block_entities: Vec<GeneratedBlockEntity>,
    pub heightmaps: Option<ColumnHeightmapSet>,
}

impl StoredColumn {
    #[must_use]
    pub fn section(&self, y: i32) -> Option<&StoredSection> {
        let index = usize::try_from(y.checked_sub(self.min_section_y)?).ok()?;
        self.sections.get(index)
    }
}

/// One section of a [`StoredColumn`]. The light is absent in a world that does not light its
/// columns.
#[derive(Clone, Debug)]
pub struct StoredSection {
    pub blocks: ChunkBlocks,
    pub biomes: BiomePalette,
    pub block_light: Option<BlockLight>,
    pub sky_light: Option<SkyLight>,
}

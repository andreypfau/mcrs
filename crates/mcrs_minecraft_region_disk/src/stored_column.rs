pub use mcrs_minecraft_anvil::ChunkStatus;
use mcrs_minecraft_anvil::{BlendingData, ChunkStructures, RetroGen, UpgradeData};
use mcrs_minecraft_block_entity::GeneratedBlockEntity;
use mcrs_minecraft_chunk::ChunkBlocks;
use mcrs_minecraft_chunk::palette::BiomePalette;
use mcrs_minecraft_chunk::{BlockLight, SkyLight};
use mcrs_minecraft_heightmap::ColumnHeightmapSet;
use mcrs_minecraft_nbt::compound::NbtCompound;

use crate::column_ticks::ColumnTicks;

/// The last version of a column, as the data layer keeps it. A client receives whole columns,
/// so Minecraft keys the data layer by column and not by section.
#[derive(Clone, Debug)]
pub struct StoredColumn {
    pub status: ChunkStatus,
    pub is_light_on: bool,
    pub inhabited_time: i64,
    pub last_update: i64,
    /// The Y of the first entry of `sections`.
    pub min_section_y: i32,
    /// Every section of the column, in ascending Y.
    pub sections: Vec<StoredSection>,
    pub block_entities: Vec<GeneratedBlockEntity>,
    /// Block entities that have no typed form. The compound stays as written; only `id`, `x`,
    /// `y` and `z` are checked.
    pub unknown_block_entities: Vec<NbtCompound>,
    pub heightmaps: ColumnHeightmapSet,
    pub retrogen: Option<RetroGen>,
    pub structures: ChunkStructures,
    pub post_processing: Vec<Vec<i16>>,
    pub upgrade_data: UpgradeData,
    pub blending_data: Option<BlendingData>,
    /// The scheduled block and fluid ticks of the column, with their delays from the game time
    /// of the write.
    pub ticks: ColumnTicks,
}

impl StoredColumn {
    /// Whether the column holds light. Only the data layer keeps light, which it lit from the
    /// blocks of the column and of the eight around it, so a kept column with light holds its
    /// final light.
    #[must_use]
    pub fn is_lit(&self) -> bool {
        self.sections
            .iter()
            .any(|section| section.block_light.is_some() || section.sky_light.is_some())
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

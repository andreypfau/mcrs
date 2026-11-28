pub use mcrs_minecraft_anvil::ChunkStatus;
use mcrs_minecraft_anvil::{BlendingData, ChunkStructures, RetroGen, UpgradeData};
use mcrs_minecraft_block_entity::GeneratedBlockEntity;
use mcrs_minecraft_chunk::ChunkBlocks;
use mcrs_minecraft_chunk::palette::BiomePalette;
use mcrs_minecraft_chunk::{BlockLight, SkyLight};
use mcrs_minecraft_heightmap::ColumnHeightmapSet;
use mcrs_minecraft_nbt::compound::NbtCompound;

use crate::column_ticks::ColumnTicks;

#[derive(Clone, Debug)]
pub struct StoredColumn {
    pub status: ChunkStatus,
    pub is_light_on: bool,
    pub inhabited_time: i64,
    pub last_update: i64,
    pub min_section_y: i32,
    pub sections: Vec<StoredSection>,
    pub block_entities: Vec<GeneratedBlockEntity>,
    pub unknown_block_entities: Vec<NbtCompound>,
    pub heightmaps: ColumnHeightmapSet,
    pub retrogen: Option<RetroGen>,
    pub structures: ChunkStructures,
    pub post_processing: Vec<Vec<i16>>,
    pub upgrade_data: UpgradeData,
    pub blending_data: Option<BlendingData>,
    pub ticks: ColumnTicks,
}

impl StoredColumn {
    #[must_use]
    pub fn is_lit(&self) -> bool {
        self.sections
            .iter()
            .any(|section| section.block_light.is_some() || section.sky_light.is_some())
    }
}

#[derive(Clone, Debug)]
pub struct StoredSection {
    pub blocks: ChunkBlocks,
    pub biomes: BiomePalette,
    pub block_light: Option<BlockLight>,
    pub sky_light: Option<SkyLight>,
}

use mcrs_minecraft_protocol::BlockStateId;

use crate::biome::source::BetaLandBiome;
use crate::block::definition::BlockDefinitions;

/// Return the (top_block, filler_block) BlockStateIds for a Beta land biome.
///
/// Mirrors BiomeBase.java fields from back2beta-server-1.7.9. Default is grass/dirt
/// (BiomeBase constructor lines 49-50). Special cases from the static block at lines 86-87:
///   Desert and IceDesert: sand/sand.
/// All other biomes: grass/dirt.
pub fn beta_surface_blocks(
    biome: BetaLandBiome,
    blocks: &BlockDefinitions,
) -> (BlockStateId, BlockStateId) {
    match biome {
        BetaLandBiome::Desert | BetaLandBiome::IceDesert => {
            let sand = blocks.default_state("minecraft:sand");
            (sand, sand)
        }
        _ => {
            let grass = blocks.default_state("minecraft:grass_block");
            let dirt = blocks.default_state("minecraft:dirt");
            (grass, dirt)
        }
    }
}

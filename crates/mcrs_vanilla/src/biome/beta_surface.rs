use mcrs_protocol::BlockStateId;

use crate::biome::source::BetaLandBiome;
use crate::block::minecraft;

/// Return the (top_block, filler_block) BlockStateIds for a Beta land biome.
///
/// Mirrors BiomeBase.java fields from back2beta-server-1.7.9. Default is grass/dirt
/// (BiomeBase constructor lines 49-50). Special cases from the static block at lines 86-87:
///   Desert and IceDesert: sand/sand.
/// All other biomes: grass/dirt.
pub fn beta_surface_blocks(biome: BetaLandBiome) -> (BlockStateId, BlockStateId) {
    match biome {
        BetaLandBiome::Desert | BetaLandBiome::IceDesert => {
            let sand = minecraft::SAND.default_state_id;
            (sand, sand)
        }
        _ => {
            let grass = minecraft::GRASS_BLOCK.default_state_id;
            let dirt = minecraft::DIRT.default_state_id;
            (grass, dirt)
        }
    }
}

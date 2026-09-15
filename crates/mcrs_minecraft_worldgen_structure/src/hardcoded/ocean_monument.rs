use bevy_math::IVec3;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_worldgen_feature::placement::HeightmapName;
use mcrs_minecraft_worldgen_feature::placer::BiomeMask;

use super::on_top_of_chunk_centre;
use crate::piece::Piece;
use crate::site::{Context, Site, Stub};

pub const SITE_IMPLIES_PIECE: Option<bool> = Some(true);

pub fn site(
    surrounding: &BiomeMask,
    ctx: &mut Context<'_>,
    _rng: &mut LegacyRandom,
) -> Option<(IVec3, Stub)> {
    let centre = IVec3::new(
        ctx.chunk.min_block_x() + 9,
        ctx.height.sea_level,
        ctx.chunk.min_block_z() + 9,
    );
    if !ctx.world.all_biomes_within(centre, 29, surrounding) {
        return None;
    }
    on_top_of_chunk_centre(ctx, HeightmapName::OceanFloorWg)
}

pub fn layout(_ctx: &mut Context<'_>, _site: Site) -> Vec<Piece> {
    Vec::new()
}

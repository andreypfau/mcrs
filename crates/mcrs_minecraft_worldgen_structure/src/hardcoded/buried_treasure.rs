use bevy_math::IVec3;
use mcrs_minecraft_core::{BlockPos, BoundingBox};
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_worldgen_feature::placement::HeightmapName;

use super::on_top_of_chunk_centre;
use crate::piece::{BuriedTreasurePiece, Piece};
use crate::site::{Context, Site, Stub};

pub const SITE_IMPLIES_PIECE: Option<bool> = Some(true);

pub fn site(ctx: &mut Context<'_>, _rng: &mut LegacyRandom) -> Option<(IVec3, Stub)> {
    on_top_of_chunk_centre(ctx, HeightmapName::OceanFloorWg)
}

pub fn layout(ctx: &mut Context<'_>, _site: Site) -> Vec<Piece> {
    let offset = BlockPos::new(
        ctx.chunk.min_block_x() + BuriedTreasurePiece::CHUNK_OFFSET,
        BuriedTreasurePiece::LAYOUT_Y,
        ctx.chunk.min_block_z() + BuriedTreasurePiece::CHUNK_OFFSET,
    );
    vec![Piece::BuriedTreasure(BuriedTreasurePiece {
        bounds: BoundingBox::point(offset),
    })]
}

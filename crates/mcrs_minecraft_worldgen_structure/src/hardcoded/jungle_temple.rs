use bevy_math::IVec3;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_worldgen_feature::placement::HeightmapName;

use super::single_piece_site;
use crate::orient::{Orientation, orient_box};
use crate::piece::{JungleTemplePiece, Piece};
use crate::site::{Context, Site, Stub};

pub const SITE_IMPLIES_PIECE: Option<bool> = Some(true);

pub fn site(ctx: &mut Context<'_>, _rng: &mut LegacyRandom) -> Option<(IVec3, Stub)> {
    single_piece_site(ctx, JungleTemplePiece::WIDTH, JungleTemplePiece::DEPTH)
}

/// The reference raises the piece at placement to the mean
/// `MOTION_BLOCKING_NO_LEAVES` height of the columns inside the first
/// decorating chunk, read from the live world; the four corners of the box
/// answer from the density column instead.
pub fn layout(ctx: &mut Context<'_>, mut site: Site) -> Vec<Piece> {
    let orientation = Orientation::random(&mut site.rng);
    let origin = IVec3::new(
        ctx.chunk.min_block_x(),
        JungleTemplePiece::LAYOUT_FLOOR,
        ctx.chunk.min_block_z(),
    );
    let bounds = orient_box(
        orientation,
        origin,
        JungleTemplePiece::WIDTH,
        JungleTemplePiece::HEIGHT,
        JungleTemplePiece::DEPTH,
    );
    let corners = [
        (bounds.min.x, bounds.min.z),
        (bounds.min.x, bounds.max.z),
        (bounds.max.x, bounds.min.z),
        (bounds.max.x, bounds.max.z),
    ];
    let height_position = corners
        .into_iter()
        .map(|(x, z)| {
            ctx.world
                .free_height(x, z, HeightmapName::MotionBlockingNoLeaves)
        })
        .sum::<i32>()
        / corners.len() as i32;
    vec![Piece::JungleTemple(JungleTemplePiece {
        bounds,
        orientation,
        height_position,
    })]
}

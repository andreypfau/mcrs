use bevy_math::IVec3;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_worldgen_feature::placement::HeightmapName;

use super::single_piece_site;
use crate::orient::{Orientation, orient_box};
use crate::piece::{DesertPyramidPiece, Piece};
use crate::site::{Context, Site, Stub};

pub const SITE_IMPLIES_PIECE: Option<bool> = Some(true);

pub fn site(ctx: &mut Context<'_>, _rng: &mut LegacyRandom) -> Option<(IVec3, Stub)> {
    single_piece_site(ctx, DesertPyramidPiece::WIDTH, DesertPyramidPiece::DEPTH)
}

/// The reference sinks the piece at placement to the lowest
/// `MOTION_BLOCKING_NO_LEAVES` height under its whole box, read from the live
/// world; the four corners of the box answer from the density column instead.
pub fn layout(ctx: &mut Context<'_>, mut site: Site) -> Vec<Piece> {
    let orientation = Orientation::random(&mut site.rng);
    let origin = IVec3::new(
        ctx.chunk.min_block_x(),
        DesertPyramidPiece::LAYOUT_FLOOR,
        ctx.chunk.min_block_z(),
    );
    let bounds = orient_box(
        orientation,
        origin,
        DesertPyramidPiece::WIDTH,
        DesertPyramidPiece::HEIGHT,
        DesertPyramidPiece::DEPTH,
    );
    let height_position = [
        (bounds.min.x, bounds.min.z),
        (bounds.min.x, bounds.max.z),
        (bounds.max.x, bounds.min.z),
        (bounds.max.x, bounds.max.z),
    ]
    .into_iter()
    .map(|(x, z)| {
        ctx.world
            .free_height(x, z, HeightmapName::MotionBlockingNoLeaves)
    })
    .min()
    .expect("four corners");
    vec![Piece::DesertPyramid(DesertPyramidPiece {
        bounds,
        orientation,
        height_position,
    })]
}

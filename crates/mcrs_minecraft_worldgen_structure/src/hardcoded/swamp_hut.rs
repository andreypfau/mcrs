use bevy_math::IVec3;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_worldgen_feature::placement::HeightmapName;

use super::on_top_of_chunk_centre;
use crate::orient::{Orientation, orient_box};
use crate::piece::{Piece, SwampHutPiece};
use crate::site::{Context, Site, Stub};

pub const SITE_IMPLIES_PIECE: Option<bool> = Some(true);

pub fn site(ctx: &mut Context<'_>, _rng: &mut LegacyRandom) -> Option<(IVec3, Stub)> {
    on_top_of_chunk_centre(ctx, HeightmapName::WorldSurfaceWg)
}

/// The reference raises the piece at placement to the mean
/// `MOTION_BLOCKING_NO_LEAVES` height under its box, read from the live world
/// of the one chunk the box lies in; the same cells answer from the density
/// columns instead.
pub fn layout(ctx: &mut Context<'_>, mut site: Site) -> Vec<Piece> {
    let orientation = Orientation::random(&mut site.rng);
    let origin = IVec3::new(
        ctx.chunk.min_block_x(),
        SwampHutPiece::LAYOUT_FLOOR,
        ctx.chunk.min_block_z(),
    );
    let bounds = orient_box(
        orientation,
        origin,
        SwampHutPiece::WIDTH,
        SwampHutPiece::HEIGHT,
        SwampHutPiece::DEPTH,
    );
    let (mut total, mut count) = (0, 0);
    for z in bounds.min.z..=bounds.max.z {
        for x in bounds.min.x..=bounds.max.x {
            total += ctx
                .world
                .free_height(x, z, HeightmapName::MotionBlockingNoLeaves);
            count += 1;
        }
    }
    vec![Piece::SwampHut(SwampHutPiece {
        bounds,
        orientation,
        height_position: total / count,
    })]
}

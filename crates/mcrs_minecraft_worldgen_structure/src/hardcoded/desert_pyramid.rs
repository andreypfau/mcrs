use bevy_math::IVec3;
use mcrs_minecraft_random::legacy::LegacyRandom;

use super::single_piece_site;
use crate::piece::Piece;
use crate::site::{Context, Site, Stub};

pub const SITE_IMPLIES_PIECE: Option<bool> = Some(true);

pub fn site(ctx: &mut Context<'_>, _rng: &mut LegacyRandom) -> Option<(IVec3, Stub)> {
    single_piece_site(ctx, 21, 21)
}

pub fn layout(_ctx: &mut Context<'_>, _site: Site) -> Vec<Piece> {
    Vec::new()
}

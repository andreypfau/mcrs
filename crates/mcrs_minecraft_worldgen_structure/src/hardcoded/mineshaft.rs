use bevy_math::IVec3;
use mcrs_minecraft_random::legacy::LegacyRandom;

use crate::MineshaftType;
use crate::piece::Piece;
use crate::site::{Context, Site, Stub};

pub const SITE_IMPLIES_PIECE: Option<bool> = None;

pub fn site(
    _mineshaft_type: MineshaftType,
    _ctx: &mut Context<'_>,
    _rng: &mut LegacyRandom,
) -> Option<(IVec3, Stub)> {
    None
}

pub fn layout(_mineshaft_type: MineshaftType, _ctx: &mut Context<'_>, _site: Site) -> Vec<Piece> {
    Vec::new()
}

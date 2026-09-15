use bevy_math::IVec3;
use mcrs_minecraft_random::legacy::LegacyRandom;

use crate::piece::Piece;
use crate::site::{Context, Site, Stub};

pub const TEMPLATES: &[&str] = &[
    "shipwreck/with_mast",
    "shipwreck/sideways_full",
    "shipwreck/sideways_fronthalf",
    "shipwreck/sideways_backhalf",
    "shipwreck/rightsideup_full",
    "shipwreck/rightsideup_fronthalf",
    "shipwreck/rightsideup_backhalf",
    "shipwreck/with_mast_degraded",
    "shipwreck/rightsideup_full_degraded",
    "shipwreck/rightsideup_fronthalf_degraded",
    "shipwreck/rightsideup_backhalf_degraded",
    "shipwreck/upsidedown_full",
    "shipwreck/upsidedown_fronthalf",
    "shipwreck/upsidedown_backhalf",
    "shipwreck/upsidedown_full_degraded",
    "shipwreck/upsidedown_fronthalf_degraded",
    "shipwreck/upsidedown_backhalf_degraded",
    "shipwreck/sideways_full_degraded",
    "shipwreck/sideways_fronthalf_degraded",
    "shipwreck/sideways_backhalf_degraded",
];

pub const SITE_IMPLIES_PIECE: Option<bool> = None;

pub fn site(
    _is_beached: bool,
    _ctx: &mut Context<'_>,
    _rng: &mut LegacyRandom,
) -> Option<(IVec3, Stub)> {
    None
}

pub fn layout(_is_beached: bool, _ctx: &mut Context<'_>, _site: Site) -> Vec<Piece> {
    Vec::new()
}

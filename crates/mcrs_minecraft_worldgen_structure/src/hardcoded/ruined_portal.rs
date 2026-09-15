use bevy_math::IVec3;
use mcrs_minecraft_random::legacy::LegacyRandom;

use crate::RuinedPortalSetup;
use crate::piece::Piece;
use crate::site::{Context, Site, Stub};

pub const TEMPLATES: &[&str] = &[
    "ruined_portal/portal_1",
    "ruined_portal/portal_2",
    "ruined_portal/portal_3",
    "ruined_portal/portal_4",
    "ruined_portal/portal_5",
    "ruined_portal/portal_6",
    "ruined_portal/portal_7",
    "ruined_portal/portal_8",
    "ruined_portal/portal_9",
    "ruined_portal/portal_10",
    "ruined_portal/giant_portal_1",
    "ruined_portal/giant_portal_2",
    "ruined_portal/giant_portal_3",
];

pub const SITE_IMPLIES_PIECE: Option<bool> = None;

pub fn site(
    _setups: &[RuinedPortalSetup],
    _ctx: &mut Context<'_>,
    _rng: &mut LegacyRandom,
) -> Option<(IVec3, Stub)> {
    None
}

pub fn layout(_setups: &[RuinedPortalSetup], _ctx: &mut Context<'_>, _site: Site) -> Vec<Piece> {
    Vec::new()
}

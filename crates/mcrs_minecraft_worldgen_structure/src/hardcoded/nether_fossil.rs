use bevy_math::IVec3;
use mcrs_minecraft_random::legacy::LegacyRandom;

use crate::piece::Piece;
use crate::site::{Context, Site, Stub};
use mcrs_minecraft_core::value_provider::HeightProvider;

pub const TEMPLATES: &[&str] = &[
    "nether_fossils/fossil_1",
    "nether_fossils/fossil_2",
    "nether_fossils/fossil_3",
    "nether_fossils/fossil_4",
    "nether_fossils/fossil_5",
    "nether_fossils/fossil_6",
    "nether_fossils/fossil_7",
    "nether_fossils/fossil_8",
    "nether_fossils/fossil_9",
    "nether_fossils/fossil_10",
    "nether_fossils/fossil_11",
    "nether_fossils/fossil_12",
    "nether_fossils/fossil_13",
    "nether_fossils/fossil_14",
];

pub const SITE_IMPLIES_PIECE: Option<bool> = None;

pub fn site(
    _height: &HeightProvider,
    _ctx: &mut Context<'_>,
    _rng: &mut LegacyRandom,
) -> Option<(IVec3, Stub)> {
    None
}

pub fn layout(_ctx: &mut Context<'_>, _site: Site) -> Vec<Piece> {
    Vec::new()
}

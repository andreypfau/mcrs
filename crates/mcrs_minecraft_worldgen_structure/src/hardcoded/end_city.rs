use bevy_math::IVec3;
use mcrs_minecraft_random::legacy::LegacyRandom;

use super::lowest_corner_site;
use crate::piece::Piece;
use crate::site::{Context, Site, Stub};

pub const TEMPLATES: &[&str] = &[
    "end_city/base_floor",
    "end_city/base_roof",
    "end_city/second_floor_1",
    "end_city/second_floor_2",
    "end_city/second_roof",
    "end_city/third_floor_1",
    "end_city/third_floor_2",
    "end_city/third_roof",
    "end_city/tower_base",
    "end_city/tower_piece",
    "end_city/tower_top",
    "end_city/bridge_piece",
    "end_city/bridge_end",
    "end_city/bridge_steep_stairs",
    "end_city/bridge_gentle_stairs",
    "end_city/ship",
    "end_city/fat_tower_base",
    "end_city/fat_tower_middle",
    "end_city/fat_tower_top",
];

pub const SITE_IMPLIES_PIECE: Option<bool> = Some(true);

pub fn site(ctx: &mut Context<'_>, rng: &mut LegacyRandom) -> Option<(IVec3, Stub)> {
    lowest_corner_site(ctx, rng)
}

pub fn layout(_ctx: &mut Context<'_>, _site: Site) -> Vec<Piece> {
    Vec::new()
}

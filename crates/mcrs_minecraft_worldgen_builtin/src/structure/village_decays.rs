use super::*;

pub fn grass_11x13(c: &mut Canvas) {
    c.variant_of([16, 1, 16], Turn::NONE, grass_16x16);
}

pub fn grass_16x16(c: &mut Canvas) {
    c.solid(&GROUND, [0, 0, 0], [15, 0, 15]);
}

pub fn grass_9x9(c: &mut Canvas) {
    c.variant_of([13, 1, 11], Turn::NONE, grass_11x13);
}

pub const TEMPLATES: &[Entry] = &[
    ("village/decays/grass_11x13", [13, 1, 11], grass_11x13),
    ("village/decays/grass_16x16", [16, 1, 16], grass_16x16),
    ("village/decays/grass_9x9", [9, 1, 9], grass_9x9),
];

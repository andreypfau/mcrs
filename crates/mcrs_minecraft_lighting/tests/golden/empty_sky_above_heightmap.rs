// A completely empty (all-air) chunk that sits above the world's heightmap;
// the sky-light engine should fill every cell with the maximum sky value of 15.

use mcrs_protocol::BlockStateId;

pub static INPUT: &[((i32, i32, i32), BlockStateId)] = &[];

pub const EXPECTED_SKY_LIGHT: [u8; 2048] = super::expected::compute_sky_field(&[]);

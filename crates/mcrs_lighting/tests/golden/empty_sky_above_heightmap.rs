// A completely empty (all-air) section that sits above the world's heightmap;
// the sky-light engine should fill every cell with the maximum sky value of 15.

use mcrs_protocol::BlockStateId;

pub static INPUT: &[((i32, i32, i32), BlockStateId)] = &[];

#[rustfmt::skip]
pub const EXPECTED_SKY_LIGHT: [u8; 2048] = [0u8; 2048];

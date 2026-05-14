// A previously placed stone block was broken; the snapshot captures the
// post-break world state where the column is empty again and the sky-light
// values below the old heightmap should have been restored.

use mcrs_protocol::BlockStateId;

pub static INPUT: &[((i32, i32, i32), BlockStateId)] = &[];

#[rustfmt::skip]
pub const EXPECTED_SKY_LIGHT: [u8; 2048] = [0u8; 2048];

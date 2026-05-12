// A single stone block is placed in a previously empty column; the heightmap
// for that column should rise to the new block and the sky-light values below
// the block should be re-occluded.

use mcrs_protocol::BlockStateId;

const STONE: BlockStateId = BlockStateId(0x1001);

pub static INPUT: &[((i32, i32, i32), BlockStateId)] = &[((8, 10, 8), STONE)];

#[rustfmt::skip]
pub const EXPECTED_SKY_LIGHT: [u8; 2048] = [0u8; 2048];

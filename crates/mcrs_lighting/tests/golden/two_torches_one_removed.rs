// Two torches were placed at opposite halves of the section and one was then
// broken; the snapshot captures the post-removal state with only the surviving
// torch contributing to the block-light field.

use mcrs_protocol::BlockStateId;

const TORCH_LIT: BlockStateId = BlockStateId(0x1000);

pub static INPUT: &[((i32, i32, i32), BlockStateId)] = &[((4, 8, 8), TORCH_LIT)];

#[rustfmt::skip]
pub const EXPECTED_BLOCK_LIGHT: [u8; 2048] = [0u8; 2048];

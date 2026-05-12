// A torch sitting on the east face of the section; downstream cross-section
// propagation should carry light into the neighbouring section along the +X
// axis. This fixture only captures the source section.

use mcrs_protocol::BlockStateId;

const TORCH_LIT: BlockStateId = BlockStateId(0x1000);

pub static INPUT: &[((i32, i32, i32), BlockStateId)] = &[((15, 8, 8), TORCH_LIT)];

#[rustfmt::skip]
pub const EXPECTED_BLOCK_LIGHT: [u8; 2048] = [0u8; 2048];

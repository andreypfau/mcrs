// A torch placed at the top face of the section, exercising the vertical
// section boundary on the +Y axis; light should propagate upward into the
// section above once the engine runs.

use mcrs_protocol::BlockStateId;

const TORCH_LIT: BlockStateId = BlockStateId(0x1000);

pub static INPUT: &[((i32, i32, i32), BlockStateId)] = &[((8, 15, 8), TORCH_LIT)];

#[rustfmt::skip]
pub const EXPECTED_BLOCK_LIGHT: [u8; 2048] = [0u8; 2048];

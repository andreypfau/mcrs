// A single lit torch placed at the centre of an otherwise empty section;
// the block-light engine should produce a radial decay field around it.

use mcrs_protocol::BlockStateId;

const TORCH_LIT: BlockStateId = BlockStateId(0x1000);

pub static INPUT: &[((i32, i32, i32), BlockStateId)] = &[((8, 8, 8), TORCH_LIT)];

pub const EXPECTED_BLOCK_LIGHT: [u8; 2048] =
    super::expected::compute_l1_attenuated_field(&[((8, 8, 8), 14)]);

// A torch sitting on the east face of the chunk; downstream cross-chunk
// propagation should carry light into the neighbouring chunk along the +X
// axis. This fixture only captures the source chunk.

use mcrs_palette::VoxelId;

const TORCH_LIT: VoxelId = VoxelId(0x1000);

pub static INPUT: &[((i32, i32, i32), VoxelId)] = &[((15, 8, 8), TORCH_LIT)];

#[rustfmt::skip]
pub const EXPECTED_BLOCK_LIGHT: [u8; 2048] = [0u8; 2048];

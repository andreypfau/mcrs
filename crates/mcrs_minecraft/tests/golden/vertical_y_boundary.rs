// A torch placed at the top face of the chunk, exercising the vertical
// chunk boundary on the +Y axis; light should propagate upward into the
// chunk above once the engine runs.

use mcrs_voxel_storage::VoxelId;

const TORCH_LIT: VoxelId = VoxelId(0x1000);

pub static INPUT: &[((i32, i32, i32), VoxelId)] = &[((8, 15, 8), TORCH_LIT)];

#[rustfmt::skip]
pub const EXPECTED_BLOCK_LIGHT: [u8; 2048] = [0u8; 2048];

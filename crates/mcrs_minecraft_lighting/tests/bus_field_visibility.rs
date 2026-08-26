// Compile-only proof that every field on `BlockPlaced` is `pub` — the
// struct literal below cannot type-check unless each named field is visible
// from this external integration test crate.

use bevy_ecs::entity::Entity;
use mcrs_voxel_math::ChunkPos;
use mcrs_voxel_math::BlockPos;
use mcrs_minecraft_block::block::BlockUpdateFlags;
use mcrs_minecraft_block::block_update::BlockPlaced;
use mcrs_palette::VoxelId;

#[test]
fn block_placed_all_fields_pub() {
    let placed = BlockPlaced {
        chunk: Entity::PLACEHOLDER,
        chunk_pos: ChunkPos::new(0, 0, 0),
        block_pos: BlockPos::new(0, 0, 0),
        old_state: VoxelId(0),
        new_state: VoxelId(1),
        flags: BlockUpdateFlags::all(),
    };
    let _chunk = placed.chunk;
    let _chunk_pos = placed.chunk_pos;
    let _block_pos = placed.block_pos;
    let _old_state = placed.old_state;
    let _new_state = placed.new_state;
    let _flags = placed.flags;
}

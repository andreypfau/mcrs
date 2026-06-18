use bevy_ecs::entity::Entity;
use bevy_ecs::message::MessageWriter;
use bevy_math::DVec3;
use mcrs_engine::world::block::BlockPos;
use mcrs_minecraft_block::block::BlockUpdateFlags;
use mcrs_minecraft_block::block_update::BlockSetRequest;
use mcrs_protocol::BlockStateId;

const OBSIDIAN: BlockStateId = BlockStateId(1126);

/// Write a 5×5 obsidian floor at `floor_y = arrival_y - 1` and clear a 3×3×3
/// air volume directly above the center. Returns the spawn position one block
/// above the floor center so the arrival dispatcher can place the entity there.
///
/// Uses `BlockSetRequest` messages so `apply_set_block_request` applies them in
/// the same tick (FixedUpdate) and chunk-sync / block-placed events fire
/// correctly. Direct `BlockPalette::set` calls are intentionally avoided.
pub fn place_end_platform(
    dim_entity: Entity,
    arrival: DVec3,
    writer: &mut MessageWriter<BlockSetRequest>,
) -> DVec3 {
    let center_x = arrival.x as i32;
    let center_z = arrival.z as i32;
    let floor_y = arrival.y as i32 - 1;

    for dx in -2i32..=2 {
        for dz in -2i32..=2 {
            writer.write(BlockSetRequest {
                dimension: dim_entity,
                pos: BlockPos::new(center_x + dx, floor_y, center_z + dz),
                new_state: OBSIDIAN,
                flags: BlockUpdateFlags::all(),
                recursion_left: 512,
            });
        }
    }

    for dy in 0..3i32 {
        for dx in -1i32..=1 {
            for dz in -1i32..=1 {
                writer.write(BlockSetRequest::remove_block(
                    dim_entity,
                    BlockPos::new(center_x + dx, floor_y + 1 + dy, center_z + dz),
                ));
            }
        }
    }

    DVec3::new(arrival.x, (floor_y + 1) as f64, arrival.z)
}

use crate::client_on_ground::ClientOnGround;
use bevy_math::DVec3;
use mcrs_minecraft_block::definition::{BlockDefinitions, BlockStateFlags};
use mcrs_minecraft_block::fluid_keys::Fluid;
use mcrs_minecraft_block_break::{Miner, rate};
use mcrs_minecraft_volume::BlockVolume;
use mcrs_voxel::{BlockPos, BlockStateId};

/// The progress that one tick adds to a dig on `state`, for a player with its eyes at `eye`.
pub(crate) fn dig_rate(
    blocks: &BlockDefinitions,
    world: &impl BlockVolume,
    state: BlockStateId,
    eye: DVec3,
    on_ground: ClientOnGround,
) -> f32 {
    let data = blocks.state(state);
    // chisle: the main hand is always empty, so its speed is 1 and it is never a correct tool;
    // the player inventory lifts it.
    let miner = Miner {
        item_speed: 1.0,
        harvests: !data
            .flags
            .contains(BlockStateFlags::REQUIRES_CORRECT_TOOL_FOR_DROPS),
        on_ground: on_ground.0,
        eyes_in_water: eyes_in_water(blocks, world, eye),
    };
    rate(data.hardness, &miner)
}

fn eyes_in_water(blocks: &BlockDefinitions, world: &impl BlockVolume, eye: DVec3) -> bool {
    world.block(BlockPos::from(eye)).is_ok_and(|state| {
        blocks
            .state(state)
            .fluid
            .is_some_and(|fluid| fluid.fluid == Fluid::Water.id())
    })
}

//! Test-only synthetic `BlockStateLightTable` factory.
//!
//! Sidesteps the deferred vanilla emitter retrofit by directly constructing
//! the `BlockStateLightTable` for state ids 0 (air), 1 (solid stone), and
//! `0x1000` (torch) — the only state ids the golden-snapshot fixtures
//! reference.

use mcrs_core::voxel_shape::VoxelShape;
use mcrs_minecraft_lighting::table::{flag_bits, BlockStateLightTable};
use mcrs_protocol::BlockStateId;

pub const SYNTH_AIR_ID: BlockStateId = BlockStateId(0);
pub const SYNTH_STONE_ID: BlockStateId = BlockStateId(1);
pub const SYNTH_TORCH_ID: BlockStateId = BlockStateId(0x1000);
pub const SYNTH_WATER_ID: BlockStateId = BlockStateId(0x1002);

pub fn synthetic_block_light_table() -> BlockStateLightTable {
    const SIZE: usize = 0x1003;

    let mut emission = vec![0u8; SIZE].into_boxed_slice();
    let mut dampening = vec![0u8; SIZE].into_boxed_slice();
    let mut occlusion: Box<[&'static VoxelShape]> =
        vec![VoxelShape::empty(); SIZE].into_boxed_slice();
    let mut flags = vec![0u8; SIZE].into_boxed_slice();

    let air = SYNTH_AIR_ID.0 as usize;
    emission[air] = 0;
    dampening[air] = 0;
    occlusion[air] = VoxelShape::empty();
    flags[air] = flag_bits::PROPAGATES_SKYLIGHT_DOWN;

    let stone = SYNTH_STONE_ID.0 as usize;
    emission[stone] = 0;
    dampening[stone] = 15;
    occlusion[stone] = VoxelShape::block();
    flags[stone] =
        flag_bits::IS_NOT_AIR | flag_bits::IS_SOLID_OPAQUE | flag_bits::IS_MOTION_BLOCKING;

    let torch = SYNTH_TORCH_ID.0 as usize;
    emission[torch] = 14;
    dampening[torch] = 0;
    occlusion[torch] = VoxelShape::empty();
    flags[torch] = flag_bits::PROPAGATES_SKYLIGHT_DOWN;

    let water = SYNTH_WATER_ID.0 as usize;
    emission[water] = 0;
    dampening[water] = 1;
    occlusion[water] = VoxelShape::empty();
    flags[water] = flag_bits::IS_NOT_AIR;

    BlockStateLightTable {
        emission,
        dampening,
        occlusion,
        flags,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthetic_table_values() {
        let table = synthetic_block_light_table();

        assert_eq!(table.emission_for(SYNTH_AIR_ID), 0);
        assert_eq!(table.emission_for(SYNTH_STONE_ID), 0);
        assert_eq!(table.emission_for(SYNTH_TORCH_ID), 14);

        assert_eq!(table.dampening_for(SYNTH_STONE_ID), 15);

        let stone_bits =
            flag_bits::IS_NOT_AIR | flag_bits::IS_SOLID_OPAQUE | flag_bits::IS_MOTION_BLOCKING;
        assert_eq!(table.flags_for(SYNTH_STONE_ID) & stone_bits, stone_bits);

        assert_ne!(
            table.flags_for(SYNTH_TORCH_ID) & flag_bits::PROPAGATES_SKYLIGHT_DOWN,
            0
        );

        assert_eq!(table.emission_for(SYNTH_WATER_ID), 0);
        assert_eq!(table.dampening_for(SYNTH_WATER_ID), 1);
        assert_eq!(
            table.flags_for(SYNTH_WATER_ID) & flag_bits::PROPAGATES_SKYLIGHT_DOWN,
            0,
            "water must not propagate skylight down (dampening != 0)"
        );
        assert_ne!(
            table.flags_for(SYNTH_WATER_ID) & flag_bits::IS_NOT_AIR,
            0,
            "water must carry IS_NOT_AIR"
        );
    }
}

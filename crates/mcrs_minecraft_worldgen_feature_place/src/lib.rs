pub mod bamboo;
pub mod blob;
pub mod block_column;
pub mod block_entity;
pub mod block_pile;
pub mod chorus_plant;
pub mod coral;
pub mod end;
pub mod entity;
pub mod fallen_tree;
pub mod fill_layer;
pub mod geode;
pub mod huge_fungus;
pub mod huge_mushroom;
pub mod iceberg;
pub mod lake;
pub mod mossy_carpet;
pub mod multiface_growth;
pub mod neighbor_spread;
pub mod ore_beta;
pub mod ore_modern;
pub mod patch;
pub mod projected_patchy_square;
pub mod replace_single_block;
pub mod room;
pub mod root_system;
pub mod scattered_ore;
pub mod sculk_patch;
pub mod simple_block;
pub mod single_block_pillar;
pub mod speleothem;
pub mod speleothem_single;
pub mod spike;
pub mod spring;
pub mod stepped_column;
pub mod tables;
pub mod template;
pub mod terrain_skin;
pub mod tree;
pub mod vines;

use fixedbitset::FixedBitSet;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use mcrs_minecraft_worldgen_feature::block_predicate::Direction;

pub fn holds(mask: &FixedBitSet, state: VoxelId) -> bool {
    mask.contains(state.0 as usize)
}

/// `Direction.getRandom`, over the declaration order `values()` hands back.
pub fn random_direction(rng: &mut XoroshiroRandom) -> Direction {
    Direction::all()[rng.next_i32_bound(6) as usize]
}

/// One bit per face, in `Direction` declaration order.
pub fn face_bit(direction: Direction) -> u8 {
    1 << direction as u8
}

//! The sight ray on a pen and on a block wall. The expected answers come from the ray of the goal
//! world over its own box table, which this ray replaced.

use glam::DVec3;
use mcrs_minecraft_physics::TestBlocks;
use mcrs_minecraft_physics::TestWorld;
use mcrs_minecraft_physics::sight_clear;
use mcrs_voxel::BlockPos;
use mcrs_voxel::BlockStateId;

const PIXEL: f64 = 1.0 / 16.0;
const RAYS: usize = 1024;

/// Bit `i % 64` of word `i / 64` is set when ray `i` of [`rays`] is clear in the pen.
const PEN_CLEAR: [u64; RAYS / 64] = [
    0x5a17_2b25_a4b5_a31d,
    0x2e00_26a4_2c68_364f,
    0x3720_2e93_3672_c317,
    0x61b4_0853_9f63_242e,
    0xb29b_00b3_2896_dc53,
    0xc637_514e_0017_a942,
    0x3274_2ab6_fe45_0694,
    0xbeaf_1256_a9bc_0a61,
    0x04aa_0faa_3049_6126,
    0xbba4_0262_bc54_a86b,
    0xe019_18e9_b797_62a0,
    0x8e9a_6410_4c27_a188,
    0xaab3_22f5_2421_37bf,
    0x773a_2a96_4b8f_04c0,
    0xe938_9723_bda9_d630,
    0x2115_30b5_7c16_2ef1,
];

/// As [`PEN_CLEAR`], for the block wall.
const WALL_CLEAR: [u64; RAYS / 64] = [
    0xbe6e_be5f_db58_ee2e,
    0xe6f3_5bff_e3b5_53fe,
    0xc9bf_7fae_fcef_7c3e,
    0x3f73_f29f_e375_67f3,
    0xbf9e_f2ac_ff7f_bedf,
    0xfff5_bbf3_fab1_fdbe,
    0xdf7f_9fdb_9fbe_baef,
    0xbb9c_6f77_ff5f_ffbd,
    0xea7d_67bb_79f6_bb3f,
    0xfd17_b8fd_fff7_cdb7,
    0x8f17_7e7f_77fc_bfb3,
    0xf7f5_f2bf_ff5b_be17,
    0x9aeb_5b59_fcdf_4fea,
    0x4f77_be35_7f7b_fbfe,
    0x0b16_baea_67ba_bd97,
    0x87ff_7e5f_fab9_6ffb,
];

fn fill(world: &mut TestWorld, min: [i32; 3], max: [i32; 3], state: BlockStateId) {
    world.fill(
        BlockPos::new(min[0], min[1], min[2]),
        BlockPos::new(max[0], max[1], max[2]),
        state,
    );
}

/// A pen of side 12 on a stone floor, with a stone fence ring 2 high and a few shapes inside.
fn pen(blocks: &TestBlocks) -> TestWorld {
    let mut world = TestWorld::new();
    fill(&mut world, [-1, -2, -1], [12, -1, 12], blocks.stone);
    fill(&mut world, [-1, 0, -1], [12, 1, -1], blocks.stone);
    fill(&mut world, [-1, 0, 12], [12, 1, 12], blocks.stone);
    fill(&mut world, [-1, 0, 0], [-1, 1, 11], blocks.stone);
    fill(&mut world, [12, 0, 0], [12, 0, 11], blocks.fence);
    fill(&mut world, [3, 0, 3], [8, 0, 3], blocks.fence);
    world.set(BlockPos::new(5, 0, 7), blocks.bottom_slab);
    world.set(BlockPos::new(6, 0, 7), blocks.stairs);
    world.set(BlockPos::new(7, 0, 7), blocks.carpet);
    world.set(BlockPos::new(8, 0, 7), blocks.off_grid);
    world.set(BlockPos::new(9, 0, 7), blocks.scaffolding);
    world.set(BlockPos::new(9, 1, 7), blocks.top_slab);
    world.set(BlockPos::new(2, 0, 9), blocks.wall);
    world
}

/// A wall across `x = 6`, 4 high, of each shape in turn along `z`, with a gap of air.
fn block_wall(blocks: &TestBlocks) -> TestWorld {
    let mut world = TestWorld::new();
    let column = [
        blocks.stone,
        blocks.bottom_slab,
        blocks.top_slab,
        blocks.stairs,
        blocks.fence,
        blocks.wall,
        blocks.carpet,
        blocks.off_grid,
        blocks.scaffolding,
        BlockStateId::AIR,
    ];
    for (z, state) in (0..).zip(column) {
        fill(&mut world, [6, 0, z], [6, 3, z], state);
    }
    world
}

/// Rays between points of the box from `(-2, -1, -2)` to `(14, 4, 14)`: a quarter with both ends
/// at one height, an eighth on the pixel grid, and the rest free.
fn rays() -> Vec<(DVec3, DVec3)> {
    let mut seed = 0x9E37_79B9_7F4A_7C15_u64;
    let mut unit = move || {
        seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (seed >> 11) as f64 / (1_u64 << 53) as f64
    };
    (0..RAYS)
        .map(|index| {
            let mut point =
                || DVec3::new(unit() * 16.0 - 2.0, unit() * 5.0 - 1.0, unit() * 16.0 - 2.0);
            let mut from = point();
            let mut to = point();
            if index % 4 == 1 {
                to.y = from.y;
            }
            if index % 8 == 3 {
                from = (from * 16.0).round() * PIXEL;
                to = (to * 16.0).round() * PIXEL;
            }
            (from, to)
        })
        .collect()
}

fn assert_answers(world: &TestWorld, blocks: &TestBlocks, expected: &[u64; RAYS / 64]) {
    for (index, (from, to)) in rays().into_iter().enumerate() {
        let word = expected.get(index / 64).copied().unwrap_or(0);
        let clear = word >> (index % 64) & 1 == 1;
        assert_eq!(
            sight_clear(world, &blocks.table, from, to),
            clear,
            "ray {index} from {from} to {to}"
        );
    }
}

#[test]
fn the_sight_ray_answers_as_before_in_a_pen() {
    let blocks = TestBlocks::new();
    assert_answers(&pen(&blocks), &blocks, &PEN_CLEAR);
}

#[test]
fn the_sight_ray_answers_as_before_through_a_block_wall() {
    let blocks = TestBlocks::new();
    assert_answers(&block_wall(&blocks), &blocks, &WALL_CLEAR);
}

#[test]
fn the_sight_ray_passes_through_powder_snow() {
    let blocks = TestBlocks::new();
    let mut world = TestWorld::new();
    fill(&mut world, [2, 0, -1], [2, 2, 1], blocks.powder_snow);
    let from = DVec3::new(0.5, 1.5, 0.5);
    let to = DVec3::new(4.5, 1.5, 0.5);
    assert!(sight_clear(&world, &blocks.table, from, to));
    world.set(BlockPos::new(3, 1, 0), blocks.stone);
    assert!(
        !sight_clear(&world, &blocks.table, from, to),
        "the stone behind the powder snow stops the same ray"
    );
}

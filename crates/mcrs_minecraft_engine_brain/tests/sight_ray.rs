//! The sight ray of the physics kernel over the collision table gives the answer of the ray of
//! the goal world over its cell table, on a pen and on a block wall.
#![expect(clippy::unwrap_used, reason = "a test fails by panicking")]

use glam::DVec3;
use mcrs_minecraft_brain::Point;
use mcrs_minecraft_engine_brain::BlockGoalWorld;
use mcrs_minecraft_engine_brain::CellBox;
use mcrs_minecraft_engine_brain::CellTable;
use mcrs_minecraft_engine_brain::Collision;
use mcrs_minecraft_engine_brain::StateCell;
use mcrs_minecraft_goals::GoalWorld;
use mcrs_minecraft_physics::TestBlocks;
use mcrs_minecraft_physics::TestWorld;
use mcrs_minecraft_physics::sight_clear;
use mcrs_minecraft_volume::BlockVolumeMut;
use mcrs_minecraft_volume::SparseVolume;
use mcrs_voxel::BlockPos;
use mcrs_voxel::BlockStateId;
use mcrs_voxel::SectionPos;

const PIXEL: f64 = 1.0 / 16.0;
const RAYS: usize = 1024;

fn cell_box(min: [f64; 3], max: [f64; 3]) -> CellBox {
    CellBox { min, max }
}

/// The boxes of the shapes of [`TestBlocks`] that the scenes use, as the cell table held them.
fn old_cells(blocks: &TestBlocks) -> CellTable {
    let shapes: Vec<(BlockStateId, Vec<CellBox>)> = vec![
        (blocks.stone, vec![cell_box([0.0; 3], [1.0; 3])]),
        (
            blocks.bottom_slab,
            vec![cell_box([0.0; 3], [1.0, 0.5, 1.0])],
        ),
        (blocks.top_slab, vec![cell_box([0.0, 0.5, 0.0], [1.0; 3])]),
        (blocks.carpet, vec![cell_box([0.0; 3], [1.0, PIXEL, 1.0])]),
        (
            blocks.fence,
            vec![cell_box(
                [6.0 * PIXEL, 0.0, 6.0 * PIXEL],
                [10.0 * PIXEL, 1.5, 10.0 * PIXEL],
            )],
        ),
        (
            blocks.wall,
            vec![cell_box(
                [4.0 * PIXEL, 0.0, 4.0 * PIXEL],
                [12.0 * PIXEL, 1.5, 12.0 * PIXEL],
            )],
        ),
        (
            blocks.stairs,
            vec![
                cell_box([0.0; 3], [1.0, 0.5, 1.0]),
                cell_box([0.0, 0.5, 0.5], [1.0; 3]),
            ],
        ),
        (blocks.off_grid, vec![cell_box([0.3; 3], [0.7, 0.45, 0.7])]),
        (
            blocks.scaffolding,
            vec![
                cell_box([0.0, 14.0 * PIXEL, 0.0], [1.0; 3]),
                cell_box([0.0; 3], [2.0 * PIXEL, 1.0, 2.0 * PIXEL]),
                cell_box([14.0 * PIXEL, 0.0, 0.0], [1.0, 1.0, 2.0 * PIXEL]),
                cell_box([0.0, 0.0, 14.0 * PIXEL], [2.0 * PIXEL, 1.0, 1.0]),
                cell_box([14.0 * PIXEL, 0.0, 14.0 * PIXEL], [1.0; 3]),
            ],
        ),
    ];
    let count = shapes.iter().map(|(id, _)| id.index() + 1).max().unwrap();
    let mut states = vec![StateCell::AIR; count];
    let mut boxes = Vec::new();
    for (id, shape) in shapes {
        let start = u32::try_from(boxes.len()).unwrap();
        boxes.extend(shape);
        let end = u32::try_from(boxes.len()).unwrap();
        states.get_mut(id.index()).unwrap().collision = Collision::Boxes { start, end };
    }
    CellTable::new(states, boxes)
}

/// The same blocks in a volume for the goal world and in a test world for the kernel.
struct Scene {
    volume: SparseVolume,
    world: TestWorld,
}

impl Scene {
    fn new() -> Self {
        let mut volume = SparseVolume::unbounded();
        for x in -1..=1 {
            for y in -1..=1 {
                for z in -1..=1 {
                    volume.insert(SectionPos::new(x, y, z), BlockStateId::AIR);
                }
            }
        }
        Self {
            volume,
            world: TestWorld::new(),
        }
    }

    fn set(&mut self, at: BlockPos, state: BlockStateId) {
        self.volume.set_block(at, state).unwrap();
        self.world.set(at, state);
    }

    fn fill(&mut self, min: [i32; 3], max: [i32; 3], state: BlockStateId) {
        for x in min[0]..=max[0] {
            for y in min[1]..=max[1] {
                for z in min[2]..=max[2] {
                    self.set(BlockPos::new(x, y, z), state);
                }
            }
        }
    }
}

/// A pen of side 12 on a stone floor, with a stone fence ring 2 high and a few shapes inside.
fn pen(blocks: &TestBlocks) -> Scene {
    let mut scene = Scene::new();
    scene.fill([-1, -2, -1], [12, -1, 12], blocks.stone);
    scene.fill([-1, 0, -1], [12, 1, -1], blocks.stone);
    scene.fill([-1, 0, 12], [12, 1, 12], blocks.stone);
    scene.fill([-1, 0, 0], [-1, 1, 11], blocks.stone);
    scene.fill([12, 0, 0], [12, 0, 11], blocks.fence);
    scene.fill([3, 0, 3], [8, 0, 3], blocks.fence);
    scene.set(BlockPos::new(5, 0, 7), blocks.bottom_slab);
    scene.set(BlockPos::new(6, 0, 7), blocks.stairs);
    scene.set(BlockPos::new(7, 0, 7), blocks.carpet);
    scene.set(BlockPos::new(8, 0, 7), blocks.off_grid);
    scene.set(BlockPos::new(9, 0, 7), blocks.scaffolding);
    scene.set(BlockPos::new(9, 1, 7), blocks.top_slab);
    scene.set(BlockPos::new(2, 0, 9), blocks.wall);
    scene
}

/// A wall across `x = 6`, 4 high, of each shape in turn along `z`, with a gap of air.
fn block_wall(blocks: &TestBlocks) -> Scene {
    let mut scene = Scene::new();
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
        scene.fill([6, 0, z], [6, 3, z], state);
    }
    scene
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

fn assert_same_answers(scene: &Scene, blocks: &TestBlocks) {
    let cells = old_cells(blocks);
    let goals = BlockGoalWorld::new(&scene.volume, &cells);
    let mut clear = 0;
    for (from, to) in rays() {
        let old = goals.line_of_sight(
            Point::new(from.x, from.y, from.z),
            Point::new(to.x, to.y, to.z),
        );
        let new = sight_clear(&scene.world, &blocks.table, from, to);
        assert_eq!(new, old, "the ray from {from} to {to}");
        clear += usize::from(old);
    }
    assert!(
        clear > RAYS / 10 && clear < RAYS * 9 / 10,
        "{clear} of {RAYS} rays are clear: the scene must block some rays and not others"
    );
}

#[test]
fn the_kernel_ray_answers_as_the_goal_ray_in_a_pen() {
    let blocks = TestBlocks::new();
    assert_same_answers(&pen(&blocks), &blocks);
}

#[test]
fn the_kernel_ray_answers_as_the_goal_ray_through_a_block_wall() {
    let blocks = TestBlocks::new();
    assert_same_answers(&block_wall(&blocks), &blocks);
}

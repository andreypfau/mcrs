use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_worldgen_feature::compile::{BlockResolver, FeatureCompileError};
use mcrs_minecraft_worldgen_feature::placer::{StateMask, WorldGenVolume, WorldStates};
use mcrs_minecraft_worldgen_feature_place::block_entity::GeneratedBlockEntity;
use mcrs_minecraft_worldgen_structure::piece::FortressKind;

use crate::canvas::{ChestStates, PieceCanvas, replaceable_by_structures};
use crate::{Oriented, state};

pub const NETHER_BRIDGE_LOOT: &str = "minecraft:chests/nether_bridge";

#[derive(Clone, Debug)]
pub struct FortressBlocks {
    pub bricks: Oriented,
    pub air: Oriented,
    pub lava: Oriented,
    pub soul_sand: Oriented,
    pub nether_wart: Oriented,
    pub fence: Oriented,
    pub fence_ns: Oriented,
    pub fence_we: Oriented,
    pub fence_nse: Oriented,
    pub fence_nsw: Oriented,
    pub fence_ne: Oriented,
    pub fence_nw: Oriented,
    pub fence_se: Oriented,
    pub fence_sw: Oriented,
    pub fence_e: Oriented,
    pub fence_w: Oriented,
    pub stairs_north: Oriented,
    pub stairs_south: Oriented,
    pub stairs_east: Oriented,
    pub stairs_west: Oriented,
    pub spawner: VoxelId,
    pub chest: ChestStates,
    pub replaceable_by_structures: StateMask,
}

impl FortressBlocks {
    pub fn compile(
        blocks: &dyn BlockResolver,
        world: &WorldStates,
    ) -> Result<Self, FeatureCompileError> {
        let oriented = |block: &str| Ok(Oriented::of(world, state(blocks, block, &[])?));
        let fence = |sides: &[&str]| {
            let on: Vec<(&str, &str)> = sides.iter().map(|side| (*side, "true")).collect();
            Ok(Oriented::of(
                world,
                state(blocks, "minecraft:nether_brick_fence", &on)?,
            ))
        };
        let stairs = |facing: &str| {
            Ok(Oriented::of(
                world,
                state(
                    blocks,
                    "minecraft:nether_brick_stairs",
                    &[("facing", facing)],
                )?,
            ))
        };
        Ok(FortressBlocks {
            bricks: oriented("minecraft:nether_bricks")?,
            air: oriented("minecraft:air")?,
            lava: oriented("minecraft:lava")?,
            soul_sand: oriented("minecraft:soul_sand")?,
            nether_wart: oriented("minecraft:nether_wart")?,
            fence: fence(&[])?,
            fence_ns: fence(&["north", "south"])?,
            fence_we: fence(&["west", "east"])?,
            fence_nse: fence(&["north", "south", "east"])?,
            fence_nsw: fence(&["north", "south", "west"])?,
            fence_ne: fence(&["north", "east"])?,
            fence_nw: fence(&["north", "west"])?,
            fence_se: fence(&["south", "east"])?,
            fence_sw: fence(&["south", "west"])?,
            fence_e: fence(&["east"])?,
            fence_w: fence(&["west"])?,
            stairs_north: stairs("north")?,
            stairs_south: stairs("south")?,
            stairs_east: stairs("east")?,
            stairs_west: stairs("west")?,
            spawner: state(blocks, "minecraft:spawner", &[])?,
            chest: ChestStates::compile(blocks)?,
            replaceable_by_structures: replaceable_by_structures(blocks, world)?,
        })
    }
}

/// `generateBox` as every fortress piece calls it: one state throughout,
/// air included.
fn solid<W: WorldGenVolume>(
    c: &mut PieceCanvas<'_, W>,
    state: &Oriented,
    min: [i32; 3],
    max: [i32; 3],
) {
    c.generate_box(min, max, state, state, false);
}

/// Each type's `postProcess` for one column.
pub fn paint_fortress<W: WorldGenVolume, R: Random>(
    b: &FortressBlocks,
    kind: FortressKind,
    c: &mut PieceCanvas<'_, W>,
    rng: &mut R,
) {
    match kind {
        FortressKind::BridgeCrossing => bridge_crossing(b, c),
        FortressKind::BridgeEndFiller { seed } => bridge_end_filler(b, c, seed),
        FortressKind::BridgeStraight => bridge_straight(b, c),
        FortressKind::CorridorStairs => corridor_stairs(b, c),
        FortressKind::CorridorBalcony => corridor_balcony(b, c),
        FortressKind::CastleEntrance => castle_entrance(b, c),
        FortressKind::SmallCorridorCrossing => small_corridor_crossing(b, c),
        FortressKind::SmallCorridorLeftTurn { chest } => small_corridor_left_turn(b, c, rng, chest),
        FortressKind::SmallCorridor => small_corridor(b, c),
        FortressKind::SmallCorridorRightTurn { chest } => {
            small_corridor_right_turn(b, c, rng, chest)
        }
        FortressKind::StalkRoom => stalk_room(b, c),
        FortressKind::MonsterThrone => monster_throne(b, c),
        FortressKind::RoomCrossing => room_crossing(b, c),
        FortressKind::StairsRoom => stairs_room(b, c),
    }
}

fn fill_down<W: WorldGenVolume>(b: &FortressBlocks, c: &mut PieceCanvas<'_, W>, x: i32, z: i32) {
    c.fill_column_down(
        &b.replaceable_by_structures,
        b.bricks.unoriented(),
        x,
        -1,
        z,
    );
}

fn bridge_crossing<W: WorldGenVolume>(b: &FortressBlocks, c: &mut PieceCanvas<'_, W>) {
    let nb = &b.bricks;
    solid(c, nb, [7, 3, 0], [11, 4, 18]);
    solid(c, nb, [0, 3, 7], [18, 4, 11]);
    solid(c, &b.air, [8, 5, 0], [10, 7, 18]);
    solid(c, &b.air, [0, 5, 8], [18, 7, 10]);
    solid(c, nb, [7, 5, 0], [7, 5, 7]);
    solid(c, nb, [7, 5, 11], [7, 5, 18]);
    solid(c, nb, [11, 5, 0], [11, 5, 7]);
    solid(c, nb, [11, 5, 11], [11, 5, 18]);
    solid(c, nb, [0, 5, 7], [7, 5, 7]);
    solid(c, nb, [11, 5, 7], [18, 5, 7]);
    solid(c, nb, [0, 5, 11], [7, 5, 11]);
    solid(c, nb, [11, 5, 11], [18, 5, 11]);
    solid(c, nb, [7, 2, 0], [11, 2, 5]);
    solid(c, nb, [7, 2, 13], [11, 2, 18]);
    solid(c, nb, [7, 0, 0], [11, 1, 3]);
    solid(c, nb, [7, 0, 15], [11, 1, 18]);
    for x in 7..=11 {
        for z in 0..=2 {
            fill_down(b, c, x, z);
            fill_down(b, c, x, 18 - z);
        }
    }
    solid(c, nb, [0, 2, 7], [5, 2, 11]);
    solid(c, nb, [13, 2, 7], [18, 2, 11]);
    solid(c, nb, [0, 0, 7], [3, 1, 11]);
    solid(c, nb, [15, 0, 7], [18, 1, 11]);
    for x in 0..=2 {
        for z in 7..=11 {
            fill_down(b, c, x, z);
            fill_down(b, c, 18 - x, z);
        }
    }
}

fn bridge_end_filler<W: WorldGenVolume>(b: &FortressBlocks, c: &mut PieceCanvas<'_, W>, seed: i32) {
    let nb = &b.bricks;
    let mut own = LegacyRandom::new(seed as i64 as u64);
    for x in 0..=4 {
        for y in 3..=4 {
            let z = own.next_i32_bound(8);
            solid(c, nb, [x, y, 0], [x, y, z]);
        }
    }
    let z = own.next_i32_bound(8);
    solid(c, nb, [0, 5, 0], [0, 5, z]);
    let z = own.next_i32_bound(8);
    solid(c, nb, [4, 5, 0], [4, 5, z]);
    for x in 0..=4 {
        let z = own.next_i32_bound(5);
        solid(c, nb, [x, 2, 0], [x, 2, z]);
    }
    for x in 0..=4 {
        for y in 0..=1 {
            let z = own.next_i32_bound(3);
            solid(c, nb, [x, y, 0], [x, y, z]);
        }
    }
}

fn bridge_straight<W: WorldGenVolume>(b: &FortressBlocks, c: &mut PieceCanvas<'_, W>) {
    let nb = &b.bricks;
    solid(c, nb, [0, 3, 0], [4, 4, 18]);
    solid(c, &b.air, [1, 5, 0], [3, 7, 18]);
    solid(c, nb, [0, 5, 0], [0, 5, 18]);
    solid(c, nb, [4, 5, 0], [4, 5, 18]);
    solid(c, nb, [0, 2, 0], [4, 2, 5]);
    solid(c, nb, [0, 2, 13], [4, 2, 18]);
    solid(c, nb, [0, 0, 0], [4, 1, 3]);
    solid(c, nb, [0, 0, 15], [4, 1, 18]);
    for x in 0..=4 {
        for z in 0..=2 {
            fill_down(b, c, x, z);
            fill_down(b, c, x, 18 - z);
        }
    }
    solid(c, &b.fence_nse, [0, 1, 1], [0, 4, 1]);
    solid(c, &b.fence_nse, [0, 3, 4], [0, 4, 4]);
    solid(c, &b.fence_nse, [0, 3, 14], [0, 4, 14]);
    solid(c, &b.fence_nse, [0, 1, 17], [0, 4, 17]);
    solid(c, &b.fence_nsw, [4, 1, 1], [4, 4, 1]);
    solid(c, &b.fence_nsw, [4, 3, 4], [4, 4, 4]);
    solid(c, &b.fence_nsw, [4, 3, 14], [4, 4, 14]);
    solid(c, &b.fence_nsw, [4, 1, 17], [4, 4, 17]);
}

fn corridor_stairs<W: WorldGenVolume>(b: &FortressBlocks, c: &mut PieceCanvas<'_, W>) {
    let nb = &b.bricks;
    for step in 0..=9 {
        let floor = (7 - step).max(1);
        let roof = (floor + 5).max(14 - step).min(13);
        solid(c, nb, [0, 0, step], [4, floor, step]);
        solid(c, &b.air, [1, floor + 1, step], [3, roof - 1, step]);
        if step <= 6 {
            c.place(&b.stairs_south, 1, floor + 1, step);
            c.place(&b.stairs_south, 2, floor + 1, step);
            c.place(&b.stairs_south, 3, floor + 1, step);
        }
        solid(c, nb, [0, roof, step], [4, roof, step]);
        solid(c, nb, [0, floor + 1, step], [0, roof - 1, step]);
        solid(c, nb, [4, floor + 1, step], [4, roof - 1, step]);
        if step & 1 == 0 {
            solid(c, &b.fence_ns, [0, floor + 2, step], [0, floor + 3, step]);
            solid(c, &b.fence_ns, [4, floor + 2, step], [4, floor + 3, step]);
        }
        for x in 0..=4 {
            fill_down(b, c, x, step);
        }
    }
}

fn corridor_balcony<W: WorldGenVolume>(b: &FortressBlocks, c: &mut PieceCanvas<'_, W>) {
    let nb = &b.bricks;
    solid(c, nb, [0, 0, 0], [8, 1, 8]);
    solid(c, &b.air, [0, 2, 0], [8, 5, 8]);
    solid(c, nb, [0, 6, 0], [8, 6, 5]);
    solid(c, nb, [0, 2, 0], [2, 5, 0]);
    solid(c, nb, [6, 2, 0], [8, 5, 0]);
    solid(c, &b.fence_we, [1, 3, 0], [1, 4, 0]);
    solid(c, &b.fence_we, [7, 3, 0], [7, 4, 0]);
    solid(c, nb, [0, 2, 4], [8, 2, 8]);
    solid(c, &b.air, [1, 1, 4], [2, 2, 4]);
    solid(c, &b.air, [6, 1, 4], [7, 2, 4]);
    solid(c, &b.fence_we, [1, 3, 8], [7, 3, 8]);
    c.place(&b.fence_se, 0, 3, 8);
    c.place(&b.fence_sw, 8, 3, 8);
    solid(c, &b.fence_ns, [0, 3, 6], [0, 3, 7]);
    solid(c, &b.fence_ns, [8, 3, 6], [8, 3, 7]);
    solid(c, nb, [0, 3, 4], [0, 5, 5]);
    solid(c, nb, [8, 3, 4], [8, 5, 5]);
    solid(c, nb, [1, 3, 5], [2, 5, 5]);
    solid(c, nb, [6, 3, 5], [7, 5, 5]);
    solid(c, &b.fence_we, [1, 4, 5], [1, 5, 5]);
    solid(c, &b.fence_we, [7, 4, 5], [7, 5, 5]);
    for z in 0..=5 {
        for x in 0..=8 {
            fill_down(b, c, x, z);
        }
    }
}

/// The shell the castle entrance and the stalk room share: walls, the fenced
/// gallery and the roof rail.
fn castle_hall<W: WorldGenVolume>(b: &FortressBlocks, c: &mut PieceCanvas<'_, W>) {
    let nb = &b.bricks;
    solid(c, nb, [0, 3, 0], [12, 4, 12]);
    solid(c, &b.air, [0, 5, 0], [12, 13, 12]);
    solid(c, nb, [0, 5, 0], [1, 12, 12]);
    solid(c, nb, [11, 5, 0], [12, 12, 12]);
    solid(c, nb, [2, 5, 11], [4, 12, 12]);
    solid(c, nb, [8, 5, 11], [10, 12, 12]);
    solid(c, nb, [5, 9, 11], [7, 12, 12]);
    solid(c, nb, [2, 5, 0], [4, 12, 1]);
    solid(c, nb, [8, 5, 0], [10, 12, 1]);
    solid(c, nb, [5, 9, 0], [7, 12, 1]);
    solid(c, nb, [2, 11, 2], [10, 12, 10]);
}

fn castle_rail<W: WorldGenVolume>(b: &FortressBlocks, c: &mut PieceCanvas<'_, W>) {
    let nb = &b.bricks;
    for i in (1..=11).step_by(2) {
        solid(c, &b.fence_we, [i, 10, 0], [i, 11, 0]);
        solid(c, &b.fence_we, [i, 10, 12], [i, 11, 12]);
        solid(c, &b.fence_ns, [0, 10, i], [0, 11, i]);
        solid(c, &b.fence_ns, [12, 10, i], [12, 11, i]);
        c.place(nb, i, 13, 0);
        c.place(nb, i, 13, 12);
        c.place(nb, 0, 13, i);
        c.place(nb, 12, 13, i);
        if i != 11 {
            c.place(&b.fence_we, i + 1, 13, 0);
            c.place(&b.fence_we, i + 1, 13, 12);
            c.place(&b.fence_ns, 0, 13, i + 1);
            c.place(&b.fence_ns, 12, 13, i + 1);
        }
    }
    c.place(&b.fence_ne, 0, 13, 0);
    c.place(&b.fence_se, 0, 13, 12);
    c.place(&b.fence_sw, 12, 13, 12);
    c.place(&b.fence_nw, 12, 13, 0);
    for z in (3..=9).step_by(2) {
        solid(c, &b.fence_nsw, [1, 7, z], [1, 8, z]);
        solid(c, &b.fence_nse, [11, 7, z], [11, 8, z]);
    }
}

fn castle_foundation<W: WorldGenVolume>(b: &FortressBlocks, c: &mut PieceCanvas<'_, W>) {
    let nb = &b.bricks;
    solid(c, nb, [4, 2, 0], [8, 2, 12]);
    solid(c, nb, [0, 2, 4], [12, 2, 8]);
    solid(c, nb, [4, 0, 0], [8, 1, 3]);
    solid(c, nb, [4, 0, 9], [8, 1, 12]);
    solid(c, nb, [0, 0, 4], [3, 1, 8]);
    solid(c, nb, [9, 0, 4], [12, 1, 8]);
    for x in 4..=8 {
        for z in 0..=2 {
            fill_down(b, c, x, z);
            fill_down(b, c, x, 12 - z);
        }
    }
    for x in 0..=2 {
        for z in 4..=8 {
            fill_down(b, c, x, z);
            fill_down(b, c, 12 - x, z);
        }
    }
}

fn castle_entrance<W: WorldGenVolume>(b: &FortressBlocks, c: &mut PieceCanvas<'_, W>) {
    let nb = &b.bricks;
    castle_hall(b, c);
    solid(c, &b.fence, [5, 8, 0], [7, 8, 0]);
    castle_rail(b, c);
    castle_foundation(b, c);
    solid(c, nb, [5, 5, 5], [7, 5, 7]);
    solid(c, &b.air, [6, 1, 6], [6, 4, 6]);
    c.place(nb, 6, 0, 6);
    c.place(&b.lava, 6, 5, 6);
}

fn small_corridor_crossing<W: WorldGenVolume>(b: &FortressBlocks, c: &mut PieceCanvas<'_, W>) {
    let nb = &b.bricks;
    solid(c, nb, [0, 0, 0], [4, 1, 4]);
    solid(c, &b.air, [0, 2, 0], [4, 5, 4]);
    solid(c, nb, [0, 2, 0], [0, 5, 0]);
    solid(c, nb, [4, 2, 0], [4, 5, 0]);
    solid(c, nb, [0, 2, 4], [0, 5, 4]);
    solid(c, nb, [4, 2, 4], [4, 5, 4]);
    small_corridor_roof(b, c);
}

fn small_corridor_roof<W: WorldGenVolume>(b: &FortressBlocks, c: &mut PieceCanvas<'_, W>) {
    solid(c, &b.bricks, [0, 6, 0], [4, 6, 4]);
    for x in 0..=4 {
        for z in 0..=4 {
            fill_down(b, c, x, z);
        }
    }
}

fn small_corridor_left_turn<W: WorldGenVolume, R: Random>(
    b: &FortressBlocks,
    c: &mut PieceCanvas<'_, W>,
    rng: &mut R,
    chest: bool,
) {
    let nb = &b.bricks;
    solid(c, nb, [0, 0, 0], [4, 1, 4]);
    solid(c, &b.air, [0, 2, 0], [4, 5, 4]);
    solid(c, nb, [4, 2, 0], [4, 5, 4]);
    solid(c, &b.fence_ns, [4, 3, 1], [4, 4, 1]);
    solid(c, &b.fence_ns, [4, 3, 3], [4, 4, 3]);
    solid(c, nb, [0, 2, 0], [0, 5, 0]);
    solid(c, nb, [0, 2, 4], [3, 5, 4]);
    solid(c, &b.fence_we, [1, 3, 4], [1, 4, 4]);
    solid(c, &b.fence_we, [3, 3, 4], [3, 4, 4]);
    if chest {
        c.create_chest(rng, &b.chest, 3, 2, 3, NETHER_BRIDGE_LOOT);
    }
    small_corridor_roof(b, c);
}

fn small_corridor<W: WorldGenVolume>(b: &FortressBlocks, c: &mut PieceCanvas<'_, W>) {
    let nb = &b.bricks;
    solid(c, nb, [0, 0, 0], [4, 1, 4]);
    solid(c, &b.air, [0, 2, 0], [4, 5, 4]);
    solid(c, nb, [0, 2, 0], [0, 5, 4]);
    solid(c, nb, [4, 2, 0], [4, 5, 4]);
    solid(c, &b.fence_ns, [0, 3, 1], [0, 4, 1]);
    solid(c, &b.fence_ns, [0, 3, 3], [0, 4, 3]);
    solid(c, &b.fence_ns, [4, 3, 1], [4, 4, 1]);
    solid(c, &b.fence_ns, [4, 3, 3], [4, 4, 3]);
    small_corridor_roof(b, c);
}

fn small_corridor_right_turn<W: WorldGenVolume, R: Random>(
    b: &FortressBlocks,
    c: &mut PieceCanvas<'_, W>,
    rng: &mut R,
    chest: bool,
) {
    let nb = &b.bricks;
    solid(c, nb, [0, 0, 0], [4, 1, 4]);
    solid(c, &b.air, [0, 2, 0], [4, 5, 4]);
    solid(c, nb, [0, 2, 0], [0, 5, 4]);
    solid(c, &b.fence_ns, [0, 3, 1], [0, 4, 1]);
    solid(c, &b.fence_ns, [0, 3, 3], [0, 4, 3]);
    solid(c, nb, [4, 2, 0], [4, 5, 0]);
    solid(c, nb, [1, 2, 4], [4, 5, 4]);
    solid(c, &b.fence_we, [1, 3, 4], [1, 4, 4]);
    solid(c, &b.fence_we, [3, 3, 4], [3, 4, 4]);
    if chest {
        c.create_chest(rng, &b.chest, 1, 2, 3, NETHER_BRIDGE_LOOT);
    }
    small_corridor_roof(b, c);
}

fn stalk_room<W: WorldGenVolume>(b: &FortressBlocks, c: &mut PieceCanvas<'_, W>) {
    let nb = &b.bricks;
    castle_hall(b, c);
    castle_rail(b, c);
    for i in 0..=6 {
        let z = i + 4;
        for x in 5..=7 {
            c.place(&b.stairs_north, x, 5 + i, z);
        }
        if (5..=8).contains(&z) {
            solid(c, nb, [5, 5, z], [7, i + 4, z]);
        } else if (9..=10).contains(&z) {
            solid(c, nb, [5, 8, z], [7, i + 4, z]);
        }
        if i >= 1 {
            solid(c, &b.air, [5, 6 + i, z], [7, 9 + i, z]);
        }
    }
    for x in 5..=7 {
        c.place(&b.stairs_north, x, 12, 11);
    }
    solid(c, &b.fence_nse, [5, 6, 7], [5, 7, 7]);
    solid(c, &b.fence_nsw, [7, 6, 7], [7, 7, 7]);
    solid(c, &b.air, [5, 13, 12], [7, 13, 12]);
    solid(c, nb, [2, 5, 2], [3, 5, 3]);
    solid(c, nb, [2, 5, 9], [3, 5, 10]);
    solid(c, nb, [2, 5, 4], [2, 5, 8]);
    solid(c, nb, [9, 5, 2], [10, 5, 3]);
    solid(c, nb, [9, 5, 9], [10, 5, 10]);
    solid(c, nb, [10, 5, 4], [10, 5, 8]);
    c.place(&b.stairs_west, 4, 5, 2);
    c.place(&b.stairs_west, 4, 5, 3);
    c.place(&b.stairs_west, 4, 5, 9);
    c.place(&b.stairs_west, 4, 5, 10);
    c.place(&b.stairs_east, 8, 5, 2);
    c.place(&b.stairs_east, 8, 5, 3);
    c.place(&b.stairs_east, 8, 5, 9);
    c.place(&b.stairs_east, 8, 5, 10);
    solid(c, &b.soul_sand, [3, 4, 4], [4, 4, 8]);
    solid(c, &b.soul_sand, [8, 4, 4], [9, 4, 8]);
    solid(c, &b.nether_wart, [3, 5, 4], [4, 5, 8]);
    solid(c, &b.nether_wart, [8, 5, 4], [9, 5, 8]);
    castle_foundation(b, c);
}

fn monster_throne<W: WorldGenVolume>(b: &FortressBlocks, c: &mut PieceCanvas<'_, W>) {
    let nb = &b.bricks;
    solid(c, &b.air, [0, 2, 0], [6, 7, 7]);
    solid(c, nb, [1, 0, 0], [5, 1, 7]);
    solid(c, nb, [1, 2, 1], [5, 2, 7]);
    solid(c, nb, [1, 3, 2], [5, 3, 7]);
    solid(c, nb, [1, 4, 3], [5, 4, 7]);
    solid(c, nb, [1, 2, 0], [1, 4, 2]);
    solid(c, nb, [5, 2, 0], [5, 4, 2]);
    solid(c, nb, [1, 5, 2], [1, 5, 3]);
    solid(c, nb, [5, 5, 2], [5, 5, 3]);
    solid(c, nb, [0, 5, 3], [0, 5, 8]);
    solid(c, nb, [6, 5, 3], [6, 5, 8]);
    solid(c, nb, [1, 5, 8], [5, 5, 8]);
    c.place(&b.fence_w, 1, 6, 3);
    c.place(&b.fence_e, 5, 6, 3);
    c.place(&b.fence_ne, 0, 6, 3);
    c.place(&b.fence_nw, 6, 6, 3);
    solid(c, &b.fence_ns, [0, 6, 4], [0, 6, 7]);
    solid(c, &b.fence_ns, [6, 6, 4], [6, 6, 7]);
    c.place(&b.fence_se, 0, 6, 8);
    c.place(&b.fence_sw, 6, 6, 8);
    solid(c, &b.fence_we, [1, 6, 8], [5, 6, 8]);
    c.place(&b.fence_e, 1, 7, 8);
    solid(c, &b.fence_we, [2, 7, 8], [4, 7, 8]);
    c.place(&b.fence_w, 5, 7, 8);
    c.place(&b.fence_e, 2, 8, 8);
    c.place(&b.fence_we, 3, 8, 8);
    c.place(&b.fence_w, 4, 8, 8);
    let pos = c.world_pos(3, 5, 5);
    if c.clip.is_inside(pos) {
        c.volume.set(pos, b.spawner);
        c.entities
            .push(GeneratedBlockEntity::mob_spawner(pos, "minecraft:blaze"));
    }
    for x in 0..=6 {
        for z in 0..=6 {
            fill_down(b, c, x, z);
        }
    }
}

fn room_crossing<W: WorldGenVolume>(b: &FortressBlocks, c: &mut PieceCanvas<'_, W>) {
    let nb = &b.bricks;
    solid(c, nb, [0, 0, 0], [6, 1, 6]);
    solid(c, &b.air, [0, 2, 0], [6, 7, 6]);
    solid(c, nb, [0, 2, 0], [1, 6, 0]);
    solid(c, nb, [0, 2, 6], [1, 6, 6]);
    solid(c, nb, [5, 2, 0], [6, 6, 0]);
    solid(c, nb, [5, 2, 6], [6, 6, 6]);
    solid(c, nb, [0, 2, 0], [0, 6, 1]);
    solid(c, nb, [0, 2, 5], [0, 6, 6]);
    solid(c, nb, [6, 2, 0], [6, 6, 1]);
    solid(c, nb, [6, 2, 5], [6, 6, 6]);
    solid(c, nb, [2, 6, 0], [4, 6, 0]);
    solid(c, &b.fence_we, [2, 5, 0], [4, 5, 0]);
    solid(c, nb, [2, 6, 6], [4, 6, 6]);
    solid(c, &b.fence_we, [2, 5, 6], [4, 5, 6]);
    solid(c, nb, [0, 6, 2], [0, 6, 4]);
    solid(c, &b.fence_ns, [0, 5, 2], [0, 5, 4]);
    solid(c, nb, [6, 6, 2], [6, 6, 4]);
    solid(c, &b.fence_ns, [6, 5, 2], [6, 5, 4]);
    for x in 0..=6 {
        for z in 0..=6 {
            fill_down(b, c, x, z);
        }
    }
}

fn stairs_room<W: WorldGenVolume>(b: &FortressBlocks, c: &mut PieceCanvas<'_, W>) {
    let nb = &b.bricks;
    solid(c, nb, [0, 0, 0], [6, 1, 6]);
    solid(c, &b.air, [0, 2, 0], [6, 10, 6]);
    solid(c, nb, [0, 2, 0], [1, 8, 0]);
    solid(c, nb, [5, 2, 0], [6, 8, 0]);
    solid(c, nb, [0, 2, 1], [0, 8, 6]);
    solid(c, nb, [6, 2, 1], [6, 8, 6]);
    solid(c, nb, [1, 2, 6], [5, 8, 6]);
    solid(c, &b.fence_ns, [0, 3, 2], [0, 5, 4]);
    solid(c, &b.fence_ns, [6, 3, 2], [6, 5, 2]);
    solid(c, &b.fence_ns, [6, 3, 4], [6, 5, 4]);
    c.place(nb, 5, 2, 5);
    solid(c, nb, [4, 2, 5], [4, 3, 5]);
    solid(c, nb, [3, 2, 5], [3, 4, 5]);
    solid(c, nb, [2, 2, 5], [2, 5, 5]);
    solid(c, nb, [1, 2, 5], [1, 6, 5]);
    solid(c, nb, [1, 7, 1], [5, 7, 4]);
    solid(c, &b.air, [6, 8, 2], [6, 8, 4]);
    solid(c, nb, [2, 6, 0], [4, 8, 0]);
    solid(c, &b.fence_we, [2, 5, 0], [4, 5, 0]);
    for x in 0..=6 {
        for z in 0..=6 {
            fill_down(b, c, x, z);
        }
    }
}

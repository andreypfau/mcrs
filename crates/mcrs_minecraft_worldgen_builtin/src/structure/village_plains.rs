use super::*;
use mcrs_minecraft_block::keys::Block;
use mcrs_minecraft_core::{Axis, Direction};
use mcrs_minecraft_worldgen_structure::blueprint::{Cell, Hinge, Patch, Slopes, top_stairs};

const OAK_STAIRS: &str = Block::OakStairs.as_static_str();
const OAK_DOOR: &str = Block::OakDoor.as_static_str();
const YELLOW_BED: &str = Block::YellowBed.as_static_str();

const OAK_STEP: &str =
    "minecraft:oak_stairs[facing=east,half=bottom,shape=straight,waterlogged=false]";
const COBBLE_STEP: &str =
    "minecraft:cobblestone_stairs[facing=east,half=bottom,shape=straight,waterlogged=false]";
const GATE_EAST: &str =
    "minecraft:oak_fence_gate[facing=east,in_wall=false,open=false,powered=false]";
const PLANKS: &str = Block::OakPlanks.as_static_str();
const FENCE: &str = Block::OakFence.as_static_str();
const OAK_LOG: &str = Block::OakLog.as_static_str();
const STRIPPED_LOG: &str = Block::StrippedOakLog.as_static_str();

const TREES: &str =
    mcrs_minecraft_worldgen_feature::keys::template_pool::VILLAGE_PLAINS_TREES.as_static_str();

const BOARDED: Slopes = Slopes::BOARDED;
const BARE: Slopes = Slopes::BARE;

impl Village {
    /// What a spot on open ground turns into once its piece is placed.
    fn turf(self) -> &'static str {
        if self.zombie { DIRT } else { GRASS }
    }
}

fn entrance(c: &mut Canvas, v: Village, at: [i32; 3], step: &str) {
    c.entrance(at, &v.pool("streets"), step);
}

kit! {
    K;
    planks: block(PLANKS),
    log: log(OAK_LOG, Y),
    log_x: log(OAK_LOG, X),
    log_z: log(OAK_LOG, Z),
    stripped: log(STRIPPED_LOG, Y),
    terracotta: block(Block::WhiteTerracotta.as_static_str()),
    yellow_pane: settled("minecraft:yellow_stained_glass_pane[waterlogged=false]"),
    white_pane: settled("minecraft:white_stained_glass_pane[waterlogged=false]"),
    fence: settled("minecraft:oak_fence[waterlogged=false]"),
    gate: block("minecraft:oak_fence_gate[facing=north,in_wall=false,open=false,powered=false]"),
    hay: log(Block::HayBlock.as_static_str(), Y),
    slab: block("minecraft:oak_slab[type=bottom,waterlogged=false]"),
    slab_top: block("minecraft:oak_slab[type=top,waterlogged=false]"),
    slab_double: block("minecraft:oak_slab[type=double,waterlogged=false]"),
    plate: block("minecraft:oak_pressure_plate[powered=false]"),
    dandelion: block(Block::Dandelion.as_static_str()),
    daisy: block(Block::OxeyeDaisy.as_static_str()),
    white_wool: block(Block::WhiteWool.as_static_str()),
    yellow_wool: block(Block::YellowWool.as_static_str()),
}

fn boards(first: bool, last: bool) -> Slopes {
    Slopes::boards([first, last])
}

fn roof(c: &mut Canvas, gable: Gable, slopes: Slopes) {
    c.roof(gable, OAK_STAIRS, None, slopes);
}

fn ridged_roof(c: &mut Canvas, gable: Gable, slopes: Slopes) {
    c.roof(gable, OAK_STAIRS, Some(&K.planks), slopes);
}

/// A cobblestone wall standing free of the block above it: a post joined low
/// to `sides`.
fn low_wall(sides: &[Direction]) -> Cell {
    wall_joined(Block::CobblestoneWall.as_static_str(), sides, &[])
}

fn table(c: &mut Canvas, at: [i32; 3]) {
    c.place(&K.fence, at[0], at[1], at[2]);
    c.place(&K.plate, at[0], at[1] + 1, at[2]);
}

fn chimney(c: &mut Canvas, at: [i32; 2], y: [i32; 2]) {
    c.solid(&S.cobble, [at[0], y[0], at[1]], [at[0], y[1] - 1, at[1]]);
    c.place(&S.cobble_wall, at[0], y[1], at[1]);
}

fn crop_plot(c: &mut Canvas, at: [i32; 3], length: i32) {
    let [x, y, z] = at;
    let rows = z..=z + length - 1;
    c.fill(&S.farmland, [x, x + 1, x + 3, x + 4], y, rows.clone());
    c.fill(&wheat(0), [x, x + 1, x + 3, x + 4], y + 1, rows.clone());
    c.fill(&S.water, x + 2, y, rows);
}

/// Four fence posts under a wool canopy between slab eaves, over a plank
/// counter `length` long on the axis `along`, with a torch on each end of the
/// counter if `lit`.
fn market_stall(
    c: &mut Canvas,
    at: [i32; 3],
    (along, length): (Axis, i32),
    wool: [&Cell; 2],
    lit: bool,
) {
    let [x, y, z] = at;
    let place = |c: &mut Canvas, cell: &Cell, u: i32, dy: i32, w: i32| {
        if along == X {
            c.place(cell, x + u, y + dy, z + w);
        } else {
            c.place(cell, x + w, y + dy, z + u);
        }
    };
    for u in 0..length {
        let end = u == 0 || u == length - 1;
        place(c, &K.planks, u, 0, 1);
        for w in 0..3 {
            let cover = if end {
                &K.slab
            } else {
                wool[((u + w) % 2) as usize]
            };
            place(c, cover, u, 3, w);
        }
        if end {
            for w in [0, 2] {
                for dy in 0..3 {
                    place(c, &K.fence, u, dy, w);
                }
            }
            if lit {
                place(c, &S.torch, u, 1, 1);
            }
        }
    }
}

/// A 5x5 room with its corner post at `at`: a cobblestone sill round a plank
/// floor, a door on the west and a window in each other wall. `flanks` stand
/// either side of the side windows and of the back one.
fn cottage(
    c: &mut Canvas,
    v: Village,
    at: [i32; 3],
    (wall, post): (&Cell, &Cell),
    flanks: [Option<&Cell>; 2],
) {
    let [x, y, z] = at;
    c.walls(&S.cobble, post, [x, y, z], [x + 4, y, z + 4]);
    c.walls(wall, post, [x, y + 1, z], [x + 4, y + 3, z + 4]);
    c.solid(&K.planks, [x + 1, y, z + 1], [x + 3, y, z + 3]);
    for side in [z, z + 4] {
        c.window(&S.pane, flanks[0], [x + 2, y + 2, side], X);
    }
    c.window(&S.pane, flanks[1], [x + 4, y + 2, z + 2], Z);
    c.door(OAK_DOOR, [x, y + 1, z + 2], East, Hinge::Right);
    villagers(c, v, PLANKS, &[[x + 2, y, z + 2]]);
}

fn street_cottage(c: &mut Canvas, v: Village, walls: (&Cell, &Cell), flanks: [Option<&Cell>; 2]) {
    cottage(c, v, [1, 0, 1], walls, flanks);
    entrance(c, v, [0, 0, 3], OAK_STEP);
}

/// A hip roof five of whose corner stairs are turned a quarter from the
/// regular ring.
fn cottage_hip_roof(c: &mut Canvas) {
    c.hip_roof(OAK_STAIRS, Some(&K.planks), [0, 4, 0], [6, 6], 3);
    c.each(&stairs(OAK_STAIRS, East), &[[1, 5, 1], [2, 6, 2]]);
    c.each(&stairs(OAK_STAIRS, South), &[[5, 5, 1], [4, 6, 2]]);
    c.place(&stairs(OAK_STAIRS, West), 4, 6, 4);
}

fn small_house_1(c: &mut Canvas, v: Village) {
    street_cottage(c, v, (&S.cobble, &K.stripped), [None, None]);
    cottage_hip_roof(c);
    c.bed(WHITE_BED, [3, 1, 2], East);
    c.place(&stairs(OAK_STAIRS, East), 4, 1, 4);
    c.place(&S.wall_torch, 4, 3, 3);
    if v.lit() {
        c.fill(&S.wall_torch, 0, 2, [2, 4]);
    }
}

fn small_house_2(c: &mut Canvas, v: Village) {
    street_cottage(c, v, (&K.terracotta, &K.stripped), [None, None]);
    for step in 0..3 {
        for i in [step, step + 1] {
            let (y, far) = (4 + step, 6 - i);
            c.walls(&K.planks, &K.planks, [i, y, i], [far, y, far]);
        }
    }
    c.bed(YELLOW_BED, [3, 1, 4], East);
    c.place(&stairs(OAK_STAIRS, West), 2, 1, 2);
    c.place(&K.planks, 3, 1, 2);
    c.place(&stairs(OAK_STAIRS, East), 4, 1, 2);
    c.fill(&S.wall_torch, [0, 4], 3, 3);
}

fn small_house_3(c: &mut Canvas, v: Village) {
    street_cottage(c, v, (&S.cobble, &K.log), [Some(&K.log_x), Some(&K.log_z)]);
    cottage_hip_roof(c);
    c.bed(YELLOW_BED, [3, 1, 2], East);
    c.place(&stairs(OAK_STAIRS, East), 4, 1, 4);
    c.fill(&S.wall_torch, [0, 4], 3, 3);
}

fn small_house_4(c: &mut Canvas, v: Village) {
    street_cottage(c, v, (&S.cobble, &K.log), [Some(&K.log_x), None]);
    c.walls(&K.planks, &K.log, [1, 4, 1], [5, 4, 5]);
    c.fill(&K.log_z, [1, 5], 4, 2..=4);
    c.fill(&S.cobble, [1, 5], 5, 2..=4);
    ridged_roof(c, Gable::new(X, [0, 6], [0, 6], 4, 3), BOARDED);
    c.walls(&K.planks, &K.planks, [2, 5, 2], [4, 5, 4]);
    c.bed(WHITE_BED, [3, 1, 2], East);
    c.solid(&stairs(COBBLE_STAIRS, South), [2, 1, 4], [4, 1, 4]);
    c.place(&S.wall_torch, 4, 3, 3);
    if v.lit() {
        c.place(&S.wall_torch, 0, 3, 3);
    } else {
        c.scatter(&S.short_grass, 0, &[[0, 0], [2, 0], [0, 4], [1, 6], [2, 6]]);
        c.scatter(&S.tall_grass, 0, &[[1, 0], [0, 5], [0, 6]]);
        c.place(&S.poppy, 0, 0, 1);
    }
}

/// Path widening by one cell a row from two cells at `[3, 0]` to four.
fn path_fan(c: &mut Canvas) {
    for z in 0..3 {
        c.solid(&S.path, [2 - z, 0, z], [3, 0, z]);
    }
}

pub(super) fn corner_01_of(c: &mut Canvas, v: Village) {
    street(c, v, &[[7, 0], [0, 8]]);
    path(c, Z, 7, [0, 7]);
    for step in 0..3 {
        c.solid(&S.path, [0, 0, 7 + step], [8 - step, 0, 7 + step]);
    }
    houses(c, v, East, 8, [6, 6]);
    decorations(c, v, GRASS, &[[10, 3], [3, 4], [13, 9], [8, 12], [3, 13]]);
}

pub(super) fn corner_02_of(c: &mut Canvas, v: Village) {
    street(c, v, &[[1, 0], [15, 14]]);
    c.place(&S.air, 9, 1, 13);
    path(c, Z, 1, [0, 12]);
    c.solid(&S.path, [1, 0, 13], [15, 0, 14]);
    c.solid(&S.path, [3, 0, 15], [15, 0, 15]);
    c.place(&S.path, 9, 0, 12);
    houses(c, v, North, 12, [9, 9]);
}

pub(super) fn corner_03_of(c: &mut Canvas, v: Village) {
    street(c, v, &[[3, 1], [1, 3]]);
    path_fan(c);
    c.solid(&S.path, [0, 0, 3], [2, 0, 3]);
}

pub(super) fn crossroad_01_of(c: &mut Canvas, v: Village) {
    street(c, v, &[[8, 0], [15, 8], [8, 15]]);
    c.each(&S.air, &[[6, 1, 6], [8, 1, 12]]);
    path(c, Z, 8, [0, 15]);
    path(c, X, 8, [7, 15]);
    houses(c, v, West, 7, [3, 7]);
    houses(c, v, West, 7, [11, 12]);
    houses(c, v, East, 9, [3, 4]);
    decorations(c, v, DIRT, &[[13, 12]]);
}

pub(super) fn crossroad_02_of(c: &mut Canvas, v: Village) {
    street(c, v, &[[8, 0], [0, 8], [15, 8], [8, 15]]);
    path(c, Z, 8, [0, 15]);
    path(c, X, 8, [0, 15]);
    decorations(c, v, GRASS, &[[5, 1], [14, 2], [3, 5], [11, 5], [13, 12]]);
}

pub(super) fn crossroad_03_of(c: &mut Canvas, v: Village) {
    street(c, v, &[[4, 0], [0, 8], [15, 8], [11, 15]]);
    path(c, Z, 4, [0, 6]);
    path(c, X, 8, [0, 15]);
    path(c, Z, 11, [10, 15]);
    houses(c, v, North, 7, [11, 11]);
    decorations(c, v, GRASS, &[[8, 11], [3, 13]]);
}

pub(super) fn crossroad_04_of(c: &mut Canvas, v: Village) {
    street(c, v, &[[2, 0], [0, 2], [2, 4]]);
    path(c, Z, 2, [0, 4]);
    c.solid(&S.path, [0, 0, 1], [0, 0, 3]);
}

pub(super) fn crossroad_05_of(c: &mut Canvas, v: Village) {
    street(c, v, &[[2, 0], [0, 2], [4, 2], [2, 4]]);
    path(c, Z, 2, [0, 4]);
    path(c, X, 2, [0, 4]);
}

pub(super) fn crossroad_06_of(c: &mut Canvas, v: Village) {
    crossroad_05_of(c, v);
    decorations(c, v, GRASS, &[[2, 2]]);
    c.place(&S.air, 2, 1, 2);
}

pub(super) fn straight_01_of(c: &mut Canvas, v: Village) {
    street(c, v, &[[7, 0], [7, 15]]);
    path(c, Z, 7, [0, 15]);
    decorations(c, v, GRASS, &[[12, 4], [11, 6], [4, 7], [2, 13], [11, 13]]);
}

/// A street along the west edge, `length` long, with places for houses on
/// its east side over `sockets`.
pub(super) fn straight_with_houses(c: &mut Canvas, v: Village, length: i32, sockets: [i32; 2]) {
    street(c, v, &[[1, 0], [1, length - 1]]);
    path(c, Z, 1, [0, length - 1]);
    houses(c, v, East, 2, sockets);
}

pub(super) fn straight_06_of(c: &mut Canvas, v: Village) {
    street(c, v, &[[8, 0], [9, 17]]);
    c.boxes(&S.air, &[([0, 1, 0], [7, 1, 0]), ([9, 1, 0], [20, 1, 0])]);
    c.scatter(
        &S.air,
        1,
        &[
            [6, 8],
            [7, 8],
            [7, 9],
            [7, 10],
            [7, 14],
            [7, 15],
            [7, 16],
            [8, 3],
            [8, 4],
        ],
    );
    c.scatter(
        &S.air,
        1,
        &[
            [8, 6],
            [8, 17],
            [9, 8],
            [9, 9],
            [9, 10],
            [9, 14],
            [9, 15],
            [11, 14],
        ],
    );
    path(c, Z, 8, [0, 5]);
    path(c, Z, 9, [6, 17]);
    houses(c, v, West, 7, [3, 4]);
    houses(c, v, East, 9, [2, 4]);
    for range in [[8, 9], [14, 15]] {
        houses(c, v, West, 8, range);
    }
    houses(c, v, East, 10, [8, 10]);
    houses(c, v, East, 10, [14, 15]);
    decorations(c, v, DIRT, &[[8, 3]]);
    decorations(c, v, GRASS, &[[9, 9], [9, 15]]);
}

pub(super) fn turn_01_of(c: &mut Canvas, v: Village) {
    street(c, v, &[[5, 0], [8, 7]]);
    for z in 0..3 {
        path(c, Z, 5 + z, [z, z]);
    }
    path(c, Z, 8, [3, 7]);
    houses(c, v, East, 9, [4, 4]);
    houses(c, v, West, 7, [6, 6]);
}

fn terminator_01(c: &mut Canvas) {
    c.jigsaw_layer(1);
    c.solid(&S.path, [1, 0, 0], [1, 0, 2]);
    c.place(&S.path, 0, 0, 1);
    c.dead_end([1, 1, 1]);
}

fn terminator_02(c: &mut Canvas) {
    c.jigsaw_layer(1);
    c.place(&S.path, 0, 0, 0);
    c.dead_end([0, 1, 0]);
}

fn terminator_03(c: &mut Canvas) {
    c.jigsaw_layer(1);
    c.boxes(&S.path, &[([0, 0, 0], [0, 0, 1]), ([2, 0, 0], [2, 0, 2])]);
    c.solid(&S.path, [1, 0, 1], [1, 0, 2]);
    c.dead_end([2, 1, 1]);
}

fn terminator_04(c: &mut Canvas) {
    c.jigsaw_layer(1);
    path_fan(c);
    c.solid(&S.path, [0, 0, 3], [1, 0, 3]);
    c.dead_end([3, 1, 1]);
}

pub fn plains_lamp_1(c: &mut Canvas) {
    c.socket_facing([1, 0, 1], "down_south", BOTTOM, EMPTY, FENCE);
    c.solid(&K.fence, [1, 1, 1], [1, 2, 1]);
    c.place(&log(Block::StrippedOakWood.as_static_str(), Y), 1, 3, 1);
    c.scatter(&S.wall_torch, 3, &[[0, 1], [2, 1], [1, 0], [1, 2]]);
}

pub fn plains_small_farm_1(c: &mut Canvas) {
    c.void([0, 2, 0], [6, 5, 8]);
    c.walls(&K.log, &K.log, [0, 0, 0], [6, 0, 8]);
    crop_plot(c, [1, 0, 1], 7);
    let sprouting = [[2, 1], [4, 1], [5, 2], [4, 3], [2, 5], [4, 6], [4, 7]];
    c.scatter(&wheat(1), 1, &sprouting);
    c.place(&wheat(2), 5, 1, 6);
    c.place(&S.dirt, 5, 0, 7);
    c.place(&S.composter, 5, 1, 7);
    entrance(c, LIVING, [0, 0, 4], OAK_LOG);
}

fn plains_large_farm_1(c: &mut Canvas) {
    c.void([0, 2, 0], [12, 5, 8]);
    c.walls(&K.log, &K.log, [0, 0, 0], [12, 0, 8]);
    c.solid(&K.log, [6, 0, 1], [6, 0, 7]);
    crop_plot(c, [1, 0, 1], 7);
    crop_plot(c, [7, 0, 1], 7);
    let ripe = Patch::rectangle([10, 1], [11, 7])
        .added(&[[8, 6], [8, 7]])
        .removed(&[[10, 3], [10, 4], [10, 6], [10, 7], [11, 7]]);
    c.scatter(&wheat(1), 1, &[[4, 2], [5, 3], [5, 4], [2, 5], [4, 5]]);
    c.scatter(&wheat(1), 1, &[[5, 6], [8, 1], [7, 3], [10, 4], [10, 6]]);
    c.patch(&wheat(7), 1, &ripe);
    c.place(&wheat(6), 8, 1, 2);
    c.place(&wheat(2), 8, 1, 3);
    c.place(&wheat(3), 10, 1, 3);
    c.place(&S.dirt, 1, 0, 1);
    c.place(&S.grass, 11, 0, 1);
    c.fill(&S.composter, [1, 11], 1, 1);
    entrance(c, LIVING, [0, 0, 4], OAK_LOG);
}

fn trapdoor(facing: Direction, half: &str) -> Cell {
    super::trapdoor(Block::OakTrapdoor.as_static_str(), facing, half, true)
}

/// A cobblestone step whose corner turns towards a neighbour that is not
/// there yet.
fn corner_step(facing: Direction, shape: &str) -> Cell {
    corner_stairs(COBBLE_STAIRS, facing, shape)
}

/// The steps either side of an entrance, their corners turned towards the
/// step the entrance becomes.
fn stoop(c: &mut Canvas, [x, y, z]: [i32; 3]) {
    c.place(&corner_step(South, "outer_left"), x, y, z - 1);
    c.place(&corner_step(North, "outer_right"), x, y, z + 1);
}

/// The lowest row of a roof slope along x, broken by dormers `planks` wide:
/// no stair in front of a dormer, an upside-down one either side of it and
/// under the row at both ends, and the plank face of the dormer one level up.
/// `at` is the height, the eave row and the row behind it.
fn dormered_eave(
    c: &mut Canvas,
    length: [i32; 2],
    at: [i32; 3],
    facing: Direction,
    dormers: &[i32],
) {
    let [y, eave, inner] = at;
    for x in length[0]..=length[1] {
        if dormers.contains(&x) {
            c.solid(&K.planks, [x - 1, y + 1, inner], [x + 1, y + 1, inner]);
        } else if dormers.contains(&(x + 1)) {
            c.place(&top_stairs(OAK_STAIRS, West), x, y, eave);
        } else if dormers.contains(&(x - 1)) {
            c.place(&top_stairs(OAK_STAIRS, East), x, y, eave);
        } else {
            c.place(&stairs(OAK_STAIRS, facing), x, y, eave);
        }
    }
    c.fill(&top_stairs(OAK_STAIRS, facing.opposite()), length, y, inner);
}

fn plains_accessory_1(c: &mut Canvas) {
    c.solid(&S.grass, [1, 0, 1], [1, 0, 3]);
    c.solid(&trapdoor(West, "bottom"), [0, 0, 1], [0, 0, 3]);
    c.solid(&trapdoor(East, "bottom"), [2, 0, 1], [2, 0, 3]);
    c.place(&trapdoor(North, "bottom"), 1, 0, 0);
    c.place(&trapdoor(South, "bottom"), 1, 0, 4);
    for (z, flower) in [(1, &S.poppy), (2, &K.daisy), (3, &K.dandelion)] {
        c.place(flower, 1, 1, z);
    }
    house_socket(c, [0, 0, 0], West, &LIVING.pool("streets"));
}

fn plains_animal_pen_1(c: &mut Canvas) {
    c.solid(&GROUND, [0, 0, 0], [4, 0, 5]);
    c.fence_ring(&K.fence, &K.gate, [0, 1, 0], [4, 5], &[[2, 0]]);
    c.place(&K.gate.turned(Turn::HALF), 2, 1, 5);
    c.entrance([0, 1, 2], EMPTY, GATE_EAST);
    c.place(&S.poppy, 1, 1, 1);
    c.scatter(&S.short_grass, 1, &[[2, 2], [3, 3]]);
    c.spot([2, 0, 1], ANIMALS, GRASS);
    c.spot([3, 0, 4], TREES, GRASS);
    open(c, 1..=7, &[]);
}

fn plains_animal_pen_2(c: &mut Canvas) {
    c.solid(&GROUND, [0, 0, 0], [6, 0, 10]);
    c.fence_ring(&K.fence, &K.gate, [0, 1, 0], [6, 10], &[]);
    c.entrance([0, 1, 5], EMPTY, GATE_EAST);
    c.scatter(&S.short_grass, 1, &[[2, 1], [3, 1], [5, 1], [1, 2], [4, 3]]);
    c.scatter(&S.short_grass, 1, &[[2, 4], [5, 5], [1, 7], [5, 8], [4, 9]]);
    c.scatter(&S.tall_grass, 1, &[[1, 1], [3, 2], [1, 9]]);
    c.place(&K.dandelion, 2, 1, 9);
    c.place(&K.hay, 3, 1, 7);
    spots(c, ANIMALS, DIRT, &[[2, 0, 3], [4, 0, 7]]);
    c.spot([4, 0, 2], TREES, DIRT);
    open(c, 1..=6, &[[4, 1, 1], [4, 1, 4], [5, 1, 4], [5, 1, 6]]);
}

fn animal_pen_3(c: &mut Canvas, v: Village) {
    c.boxes(&GROUND, &[([2, 0, 0], [6, 0, 1]), ([0, 0, 2], [6, 0, 2])]);
    c.boxes(&GROUND, &[([0, 0, 3], [7, 0, 8]), ([3, 0, 9], [7, 0, 10])]);
    c.boxes(&S.water, &[([4, 0, 7], [6, 0, 7]), ([5, 0, 6], [5, 0, 8])]);

    c.boxes(&K.fence, &[([2, 1, 0], [6, 1, 0]), ([2, 1, 1], [2, 1, 2])]);
    c.boxes(&K.fence, &[([6, 1, 1], [6, 1, 3]), ([0, 1, 2], [1, 1, 2])]);
    c.boxes(&K.fence, &[([0, 1, 3], [0, 1, 8]), ([7, 1, 3], [7, 1, 10])]);
    c.boxes(&K.fence, &[([1, 1, 8], [3, 1, 8]), ([3, 1, 9], [3, 1, 10])]);
    c.solid(&K.fence, [4, 1, 10], [6, 1, 10]);
    c.entrance([0, 1, 5], EMPTY, GATE_EAST);
    c.fill(&S.torch, 0, 2, [4, 6]);
    spots(c, ANIMALS, v.turf(), &[[2, 0, 5], [4, 0, 1], [5, 0, 5]]);
    decorations(c, v, v.turf(), &[[0, 0], [1, 10], [3, 7], [4, 3]]);
    open(c, 0..=0, &[]);
    open(c, 3..=5, &[]);
}

fn small_house_6(c: &mut Canvas, v: Village) {
    let (wall, post) = (&K.planks, &K.log);
    c.solid(&S.cobble, [2, 0, 1], [5, 0, 5]);
    c.posts(post, &[2, 5], &[1, 5], [0, 0]);
    c.walls(wall, post, [2, 1, 1], [5, 3, 5]);
    c.walls(&K.log_x, post, [2, 4, 1], [5, 4, 5]);
    c.fill(&K.log_z, [2, 5], 4, 2..=4);
    c.fill(wall, [2, 5], 5, 2..=4);
    ridged_roof(c, Gable::new(X, [0, 6], [1, 6], 4, 3), BOARDED);
    c.place(&S.pane, 5, 2, 3);
    c.door(OAK_DOOR, [2, 1, 3], East, Hinge::Left);
    c.place(&stairs(COBBLE_STAIRS, East), 1, 0, 3);
    entrance(c, v, [0, 0, 3], NOTHING);
    for (z, edge, end) in [(2, North, 1), (4, South, 5)] {
        c.place(&S.grass, 1, 0, z);
        c.place(&S.poppy, 1, 1, z);
        c.place(&trapdoor(West, "top"), 0, 0, z);
        c.place(&trapdoor(edge, "top"), 1, 0, end);
    }
    c.bed(WHITE_BED, [4, 1, 3], South);
    c.place(wall, 4, 1, 2);
    c.place(&S.torch, 4, 2, 2);
    if v.lit() {
        c.place(&S.wall_torch, 1, 3, 3);
    }
    villagers(c, v, COBBLE, &[[3, 0, 4]]);
}

fn small_house_7(c: &mut Canvas, v: Village) {
    let (wall, post) = (&S.cobble, &K.log);
    c.walls(wall, post, [1, 0, 1], [5, 3, 6]);
    c.solid(&K.planks, [2, 0, 2], [4, 0, 5]);
    c.posts(post, &[2, 4], &[2, 5], [0, 0]);
    for z in [1, 6] {
        c.window(&S.pane, Some(&K.log_x), [3, 2, z], X);
    }
    c.fill(&K.log_z, [1, 5], 2, [2, 5]);
    c.solid(&S.pane, [5, 2, 3], [5, 2, 4]);
    c.fill(wall, [1, 5], 4, 2..=5);
    c.fill(wall, [1, 5], 5, 3..=4);
    roof(c, Gable::new(X, [0, 7], [0, 6], 3, 4), BOARDED);
    c.door(OAK_DOOR, [1, 1, 3], East, Hinge::Left);
    c.door(OAK_DOOR, [1, 1, 4], East, Hinge::Right);
    c.place(&stairs(COBBLE_STAIRS, South), 0, 0, 2);
    c.place(&stairs(COBBLE_STAIRS, East), 0, 0, 3);
    c.place(&corner_step(North, "outer_right"), 0, 0, 5);
    entrance(c, v, [0, 0, 4], COBBLE_STEP);
    c.bed(YELLOW_BED, [3, 1, 2], West);
    c.place(&stairs(OAK_STAIRS, North), 4, 1, 2);
    chest(c, [4, 1, 5], North, "village_plains_house");
    c.fill(&S.wall_torch, 2, 3, [3, 4]);
    if v.lit() {
        c.fill(&S.wall_torch, 0, 2, [2, 5]);
    }
    villagers(c, v, PLANKS, &[[3, 0, 4]]);
}

fn small_house_8(c: &mut Canvas, v: Village) {
    let (wall, post) = (&S.cobble, &K.log);
    for y in 0..2 {
        c.patch(&GROUND, y, &Patch::diamond([3, 4], 4));
    }
    c.fill(&S.water, 3, 1, [1, 7]);
    c.fill(&S.farmland, [2, 4], 1, [1, 7]);
    c.fill(&wheat(7), [2, 4], 2, [1, 7]);
    c.place(&S.water, 6, 1, 4);
    c.fill(&S.farmland, 6, 1, [3, 5]);
    c.fill(&wheat(7), 6, 2, [3, 5]);
    cottage(c, v, [1, 2, 2], (wall, post), [None, None]);
    stoop(c, [0, 2, 4]);
    c.entrance([0, 2, 4], EMPTY, COBBLE_STEP);
    c.walls(wall, post, [1, 6, 2], [5, 6, 6]);
    c.fill(&K.log_z, [1, 5], 6, 3..=5);
    c.fill(wall, [1, 5], 7, 3..=5);
    ridged_roof(c, Gable::new(X, [1, 7], [0, 6], 6, 3), BOARDED);
    c.place(&K.slab_double, 0, 8, 4);
    for (eave, inner, facing) in [(2, 3, South), (6, 5, North)] {
        c.window(&S.pane, Some(&K.planks), [3, 7, eave], X);
        c.solid(&K.planks, [2, 8, inner], [4, 8, inner]);
        c.place(&stairs(OAK_STAIRS, East), 2, 8, eave);
        c.place(&stairs(OAK_STAIRS, facing), 3, 8, eave);
        c.place(&stairs(OAK_STAIRS, West), 4, 8, eave);
    }
    c.bed(WHITE_BED, [3, 3, 3], East);
    c.place(&stairs(OAK_STAIRS, East), 4, 3, 5);
    c.each(&S.wall_torch, &[[0, 5, 4], [4, 6, 4]]);
    let kept = [
        [0, 0, 0],
        [1, 0, 8],
        [6, 0, 0],
        [7, 0, 0],
        [0, 1, 0],
        [0, 1, 1],
        [1, 1, 0],
        [6, 1, 0],
        [7, 1, 0],
    ];
    open(c, 0..=1, &kept);
}

fn medium_house_2(c: &mut Canvas, v: Village) {
    let (wall, post) = (&S.cobble, &K.log);
    let valley = 6;
    c.walls(wall, post, [1, 0, 1], [5, 3, 11]);
    c.posts(post, &[1, 5], &[5, 7], [0, 3]);
    c.solid(&K.planks, [2, 0, 2], [4, 0, 10]);
    c.solid(wall, [2, 0, valley], [4, 0, valley]);
    ridged_roof(c, Gable::new(X, [0, 6], [0, 6], 3, 3), BOARDED.from(0, 1));
    ridged_roof(c, Gable::new(X, [6, 12], [0, 6], 3, 3), BOARDED.from(1, 0));
    c.fill(&K.log_z, [1, 5], 4, [2..=4, 8..=10]);
    c.solid(&K.slab_double, [0, 4, valley], [6, 4, valley]);
    c.fill(&K.slab_top, [0, 6], 3, valley);
    c.fill(wall, 3, 0, [3, 9]);
    c.fill(&S.pane, [1, 5], 2, [3, 9]);
    c.fill(&S.pane, 3, 2, [1, 11]);
    c.door(OAK_DOOR, [1, 1, valley], East, Hinge::Right);
    entrance(c, v, [0, 0, valley], COBBLE_STEP);
    c.bed(YELLOW_BED, [2, 1, 3], North);
    c.bed(YELLOW_BED, [4, 1, 9], South);
    c.place(&stairs(OAK_STAIRS, North), 4, 1, 2);
    table(c, [4, 1, 3]);
    chest(c, [2, 1, 10], North, "village_plains_house");
    c.fill(&S.wall_torch, 4, 2, [5, 7]);
    if v.lit() {
        c.fill(&S.wall_torch, 0, 2, [5, 7]);
        c.place(&S.wall_torch, 6, 2, valley);
    }
    villagers(c, v, COBBLE, &[[3, 0, 4], [3, 0, 8]]);
}

fn plains_armorer_house_1(c: &mut Canvas) {
    let (wall, post) = (&S.cobble, &K.log);
    let smooth = block(Block::SmoothStone.as_static_str());
    let bricks = block(Block::Bricks.as_static_str());
    c.solid(wall, [1, 0, 1], [7, 0, 6]);
    c.walls(wall, post, [1, 0, 1], [7, 3, 6]);
    c.fill(wall, [1, 7], 4, 2..=5);
    c.fill(wall, [1, 7], 5, 3..=4);
    roof(c, Gable::new(X, [0, 7], [0, 8], 3, 4), BOARDED);
    for x in [3, 5] {
        c.window(&S.pane, Some(&K.log_x), [x, 2, 1], X);
    }
    c.fill(&S.pane, 7, 2, [3, 4]);
    c.fill(&K.log_z, 7, 2, [2, 5]);
    c.door(OAK_DOOR, [1, 1, 3], East, Hinge::Left);
    c.door(OAK_DOOR, [1, 1, 4], East, Hinge::Right);
    c.place(&stairs(COBBLE_STAIRS, East), 0, 0, 3);
    c.entrance([0, 0, 4], EMPTY, COBBLE_STEP);

    c.boxes(&smooth, &[([2, 1, 2], [6, 1, 2]), ([6, 1, 3], [6, 1, 5])]);

    c.place(&bricks, 4, 0, 5);
    c.furnace([4, 1, 6], "blast_furnace", North);
    c.place(&bricks, 4, 2, 6);
    chimney(c, [4, 6], [3, 7]);
    for (x, facing) in [(3, East), (5, West)] {
        c.solid(wall, [x, 1, 5], [x, 2, 5]);
        c.place(&stairs(COBBLE_STAIRS, facing), x, 3, 5);
    }
    c.place(wall, 4, 3, 5);
    c.place(&stairs(COBBLE_STAIRS, South), 4, 4, 5);

    c.solid(wall, [3, 0, 7], [5, 1, 7]);
    c.place(&stairs(COBBLE_STAIRS, East), 3, 2, 7);
    c.place(&stairs(COBBLE_STAIRS, North), 4, 2, 7);
    c.place(&stairs(COBBLE_STAIRS, West), 5, 2, 7);
    c.place(&top_stairs(OAK_STAIRS, West), 2, 3, 7);
    c.solid(&K.slab_top, [3, 3, 7], [5, 3, 7]);
    c.place(&top_stairs(OAK_STAIRS, East), 6, 3, 7);

    c.place(&S.wall_torch, 4, 2, 2);
    c.fill(&S.wall_torch, 0, 2, [2, 5]);
    c.fill(&wall_torch(North), [2, 6], 4, 5);
}

fn plains_cartographer_1(c: &mut Canvas) {
    let (wall, post) = (&S.cobble, &K.log);
    let white = block(Block::WhiteCarpet.as_static_str());
    let yellow = block(Block::YellowCarpet.as_static_str());
    c.solid(&GROUND, [1, 0, 1], [8, 0, 5]);
    c.solid(&K.planks, [3, 0, 2], [7, 0, 4]);
    c.place(wall, 2, 0, 3);
    c.solid(&S.path, [0, 0, 3], [1, 0, 3]);
    c.walls(wall, post, [2, 1, 1], [8, 3, 5]);
    c.walls(post, post, [2, 4, 1], [8, 4, 5]);
    c.fill(wall, [2, 8], 5, 2..=4);
    ridged_roof(c, Gable::new(X, [0, 6], [1, 9], 4, 3), BOARDED);
    c.solid(&K.slab, [1, 7, 3], [9, 7, 3]);
    c.fill(&S.pane, [4, 6], 2, [1, 5]);
    c.place(&S.pane, 8, 2, 3);
    c.door(OAK_DOOR, [2, 1, 3], East, Hinge::Left);
    entrance(c, LIVING, [0, 1, 3], NOTHING);

    for (north, south, edge, facing) in [(1, 2, 0, North), (4, 5, 6, South)] {
        c.fill(&S.grass, 1, 1, [north, south]);
        c.fill(&trapdoor(West, "bottom"), 0, 1, [north, south]);
        c.place(&trapdoor(facing, "bottom"), 1, 1, edge);
        c.place(&K.dandelion, 1, 2, north);
        c.place(&S.poppy, 1, 2, south);
    }

    checker(c, [&white, &yellow], 1, [5, 2], [7, 4]);
    c.place(&block(Block::CartographyTable.as_static_str()), 6, 1, 3);
    table(c, [3, 1, 2]);
    chest(c, [3, 1, 4], North, "village_cartographer");
    c.fill(&S.wall_torch, [1, 3, 7], 4, 3);
    open(c, 0..=0, &[]);
}

fn plains_masons_house_1(c: &mut Canvas) {
    let (wall, post) = (&K.terracotta, &K.log);
    let clay = block(Block::Clay.as_static_str());
    let fired = block(Block::Terracotta.as_static_str());
    c.boxes(&S.cobble, &[([2, 0, 1], [6, 0, 7]), ([0, 0, 4], [1, 0, 7])]);
    c.place(&S.cobble, 1, 0, 3);
    c.solid(&K.planks, [4, 0, 3], [4, 0, 5]);
    c.walls(wall, post, [2, 1, 1], [6, 3, 7]);
    c.fill(&K.log_z, [2, 6], 4, 2..=6);
    c.fill(wall, [2, 6], 5, 3..=5);
    ridged_roof(c, Gable::new(X, [0, 8], [1, 7], 3, 4), BOARDED);
    c.scatter(&S.pane, 2, &[[4, 1], [4, 7], [6, 3], [2, 5], [6, 5]]);
    c.door(OAK_DOOR, [2, 1, 3], East, Hinge::Left);
    entrance(c, LIVING, [0, 0, 3], COBBLE_STEP);

    c.place(&S.grass, 1, 0, 2);
    c.place(&K.dandelion, 1, 1, 2);
    c.place(&trapdoor(West, "top"), 0, 0, 2);
    c.place(&trapdoor(North, "top"), 1, 0, 1);
    c.solid(&K.fence, [0, 1, 4], [0, 1, 7]);
    c.place(&K.fence, 1, 1, 7);
    c.fill(&S.torch, 0, 2, [4, 7]);

    c.boxes(&clay, &[([4, 1, 2], [5, 1, 2]), ([5, 1, 2], [5, 2, 2])]);
    c.place(&clay, 5, 1, 3);
    c.each(&fired, &[[5, 1, 4], [5, 3, 2]]);
    c.place(&block("minecraft:stonecutter[facing=north]"), 4, 1, 6);
    c.fill(&S.wall_torch, [3, 5], 4, 4);
}

fn plains_fisher_cottage_1(c: &mut Canvas) {
    let (wall, post) = (&S.cobble, &K.log);
    let wet_fence = settled("minecraft:oak_fence[waterlogged=true]");
    let mound = Patch::diamond([5, 5], 6)
        .added(&[[0, 7], [10, 7], [8, 9]])
        .removed(&[[9, 3], [2, 8], [3, 9]]);
    for y in 0..2 {
        c.patch(&GROUND, y, &mound);
    }
    c.rows(&S.water, 1, 1, &[[4, 6], [3, 7], [6, 7], [6, 8]]);
    c.rows(&S.water, 1, 5, &[[8, 9], [6, 9], [6, 8], [4, 7]]);
    c.solid(wall, [2, 1, 4], [4, 1, 4]);
    c.solid(&wet_fence, [6, 1, 5], [7, 1, 5]);

    c.walls(wall, post, [1, 2, 3], [5, 5, 7]);
    c.solid(&S.water, [2, 2, 4], [4, 2, 4]);
    c.solid(&K.planks, [2, 2, 5], [4, 2, 6]);
    c.walls(&K.log_x, post, [1, 6, 3], [5, 6, 7]);
    c.fill(&K.log_z, [1, 5], 6, 4..=6);
    c.fill(wall, [1, 5], 7, 4..=6);
    ridged_roof(c, Gable::new(X, [2, 8], [0, 6], 6, 3), BOARDED);
    c.fill(&S.pane, 3, 4, [3, 7]);
    c.door(OAK_DOOR, [1, 3, 5], East, Hinge::Left);
    c.door(OAK_DOOR, [5, 3, 5], West, Hinge::Right);
    c.place(&stairs(COBBLE_STAIRS, South), 0, 2, 4);
    c.place(&stairs(COBBLE_STAIRS, East), 0, 2, 5);
    c.place(&stairs(COBBLE_STAIRS, North), 0, 2, 6);
    c.entrance([0, 2, 7], EMPTY, NOTHING);
    c.place(&stairs(OAK_STAIRS, West), 6, 2, 5);
    c.place(&K.slab, 7, 2, 5);

    c.solid(&S.dirt, [6, 0, 5], [7, 0, 5]);
    c.solid(&trapdoor(South, "bottom"), [2, 3, 5], [4, 3, 5]);
    chest(c, [4, 3, 6], West, "village_fisher");
    for at in [[2, 2, 2], [9, 2, 7]] {
        c.barrel(at, Direction::Up);
    }
    c.fill(&S.wall_torch, [0, 6], 5, 5);
    open(c, 0..=1, &[[10, 1, 1]]);
}

fn plains_temple_3(c: &mut Canvas) {
    let (wall, post) = (&K.terracotta, &K.log);
    c.walls(&S.cobble, post, [1, 0, 1], [9, 0, 5]);
    c.solid(&K.planks, [2, 0, 2], [8, 0, 4]);
    c.walls(wall, post, [1, 1, 1], [9, 4, 5]);
    c.posts(post, &[5], &[1, 5], [0, 4]);
    c.fill(post, [1, 9], 4, [2, 4]);
    c.fill(&K.log_z, [1, 9], 5, 2..=4);

    ridged_roof(c, Gable::new(X, [1, 5], [0, 10], 5, 2), BOARDED);
    for (eave, inner, facing) in [(0, 1, South), (6, 5, North)] {
        dormered_eave(c, [0, 10], [4, eave, inner], facing, &[3, 7]);
        c.fill(&stairs(OAK_STAIRS, East), [2, 6], 5, eave);
        c.fill(&stairs(OAK_STAIRS, facing), [3, 7], 5, eave);
        c.fill(&stairs(OAK_STAIRS, West), [4, 8], 5, eave);
        c.fill(&K.yellow_pane, [3, 7], 2, inner);
        c.fill(&K.white_pane, [3, 7], 3, inner);
    }
    c.place(&K.yellow_pane, 9, 2, 3);

    c.door(OAK_DOOR, [1, 1, 3], East, Hinge::Right);
    stoop(c, [0, 0, 3]);
    entrance(c, LIVING, [0, 0, 3], COBBLE_STEP);
    for z in [1, 5] {
        c.torch_post(&S.cobble_wall, [0, 0, z], 1);
    }

    c.solid(&stairs(OAK_STAIRS, North), [2, 1, 2], [5, 1, 2]);
    c.solid(&stairs(OAK_STAIRS, South), [2, 1, 4], [5, 1, 4]);
    c.brewing_stand([7, 1, 3]);
    c.fill(&S.wall_torch, [2, 8], 4, 3);
    c.place(&S.wall_torch, 10, 3, 3);
}

fn small_house_5(c: &mut Canvas, v: Village) {
    let post = &K.log;
    let ladder = block("minecraft:ladder[facing=south,waterlogged=false]");
    let carpet = block(Block::GreenCarpet.as_static_str());
    for reach in 0..=3 {
        c.fill(&S.dirt, reach..=7, 0, [4 - reach, 4 + reach]);
    }

    c.walls(&S.cobble, post, [3, 1, 1], [7, 1, 7]);
    c.fill(&K.planks, 4..=6, 2..=3, [1, 7]);
    c.solid(&K.planks, [7, 2, 2], [7, 3, 6]);
    c.walls(&K.planks, post, [3, 4, 1], [7, 7, 7]);
    c.posts(post, &[3, 7], &[1, 7], [1, 7]);
    c.fill(&K.log_x, 4..=6, 5, [1, 7]);
    c.fill(&S.pane, 5, 3, [1, 7]);
    for y in [3, 6] {
        c.window(&S.pane, Some(post), [7, y, 4], Z);
    }
    c.fill(&S.cobble, 2, 1..=4, [2, 6]);
    c.solid(&S.cobble, [1, 1, 3], [1, 4, 5]);
    c.boxes(&K.planks, &[([3, 1, 2], [6, 1, 6]), ([2, 1, 3], [2, 1, 5])]);
    c.boxes(&K.planks, &[([2, 4, 3], [4, 4, 5]), ([3, 4, 6], [4, 4, 6])]);
    c.place(&K.planks, 3, 4, 2);

    c.fill(&K.log_z, [3, 7], 8, 2..=6);
    c.fill(&K.planks, 4..=6, 8, [2, 6]);
    c.walls(&K.planks, &K.planks, [3, 9, 3], [7, 9, 5]);
    ridged_roof(c, Gable::new(X, [0, 8], [2, 8], 7, 4), BOARDED);

    c.door(OAK_DOOR, [1, 2, 4], East, Hinge::Left);
    entrance(c, v, [0, 1, 4], COBBLE_STEP);
    c.open_door(
        Block::OakDoor.as_static_str(),
        [3, 5, 3],
        South,
        Hinge::Right,
    );
    c.door(OAK_DOOR, [3, 5, 5], East, Hinge::Right);
    c.solid(&K.fence, [1, 5, 3], [1, 5, 5]);
    c.fill(&K.fence, 2, 5, [2, 6]);

    c.solid(&ladder, [4, if v.zombie { 3 } else { 2 }, 2], [4, 4, 2]);
    c.bed(WHITE_BED, [5, 2, 2], East);
    if v.zombie {
        c.fill(&carpet, 5..=6, 2, 3);
    } else {
        c.fill(&carpet, 4, 2, 4..=5);
    }
    c.fill(&stairs(OAK_STAIRS, East), 6, 2, [5, 6]);
    table(c, [5, 2, 6]);
    c.place(&wall_torch(East), 2, 3, 3);
    c.fill(&S.wall_torch, [4, 6], 7, 4);
    if v.lit() {
        c.place(&wall_torch(East), 2, 3, 5);
        c.place(&S.wall_torch, 0, 4, 4);
        c.fill(&S.wall_torch, 8, [3, 6], [3, 5]);
    }
    villagers(c, v, PLANKS, &[[3, 1, 2], [3, 1, 5]]);
    decorations(c, v, DIRT, &[[0, 1], [1, 7], [8, 3]]);
    open(c, 0..=0, &[[8, 0, 7]]);
}

fn plains_library_2(c: &mut Canvas) {
    let post = &K.log;
    c.solid(&S.dirt, [2, 0, 1], [6, 0, 7]);
    c.solid(&S.cobble, [3, 0, 2], [5, 0, 6]);
    c.solid(&S.dirt, [5, 0, 4], [5, 0, 5]);
    c.fill(&S.cobble, 2, 0, [3, 5]);
    c.fill(&S.path, 0..=1, 0, [3, 5]);
    c.fill(&S.grass, 0, 0, [2, 6]);
    c.place(&S.dirt, 0, 0, 4);

    c.walls(&S.cobble, post, [2, 1, 1], [6, 3, 7]);
    c.walls(&K.log_x, &K.log_z, [2, 4, 1], [6, 4, 7]);
    c.fill(&K.log_z, [2, 6], 4, 2..=6);
    c.fill(&K.log_z, [2, 6], 8, 3..=5);
    c.walls(&K.planks, post, [2, 5, 1], [6, 7, 7]);
    c.posts(&K.planks, &[2, 6], &[1, 7], [7, 7]);
    c.solid(&K.planks, [3, 4, 2], [4, 4, 6]);
    c.place(&K.planks, 5, 4, 6);

    ridged_roof(c, Gable::new(X, [1, 7], [1, 7], 7, 3), BOARDED);
    c.solid(&K.log_x, [1, 9, 4], [7, 9, 4]);
    for (eave, inner, facing) in [(0, 1, South), (8, 7, North)] {
        dormered_eave(c, [1, 7], [6, eave, inner], facing, &[4]);
        c.solid(&stairs(OAK_STAIRS, facing), [3, 7, eave], [5, 7, eave]);
        c.place(&S.wall_torch, 4, 4, eave);
        c.fill(&S.pane, 4, [2, 6], inner);
    }
    c.fill(&S.pane, 6, [2, 6], [3, 5]);

    for (z, hinge) in [(3, Hinge::Left), (5, Hinge::Right)] {
        c.door(OAK_DOOR, [2, 1, z], East, hinge);
        c.door(OAK_DOOR, [2, 5, z], East, hinge);
    }
    c.fill(&S.wall_torch, 1, [3, 7], [3, 5]);
    c.fill(&K.fence, 0, 3, [3, 5]);
    c.fill(&K.fence, 0, 1..=3, [2, 4, 6]);
    entrance(c, LIVING, [0, 1, 4], FENCE);
    c.solid(&K.log_z, [0, 4, 2], [0, 4, 6]);
    c.solid(&K.planks, [1, 4, 2], [1, 4, 6]);
    c.solid(&K.fence, [0, 5, 2], [0, 5, 6]);
    c.fill(&K.log_x, 1, 4, [1, 7]);
    c.fill(&K.fence, 1, 5, [1, 7]);

    for step in 0..4 {
        c.place(&stairs(OAK_STAIRS, North), 5, 1 + step, 5 - step);
    }
    c.place(&K.planks, 5, 1, 4);
    c.solid(&S.bookshelf, [5, 1, 2], [5, 3, 2]);
    c.place(&S.bookshelf, 5, 1, 3);
    c.lectern([3, 1, 2], South);
    c.fill(&S.wall_torch, [3, 7], 2, 4);
    c.fill(&S.wall_torch, [5, 7], 6, 4);
    open(c, 0..=0, &[]);
    c.void([7, 1, 8], [7, 1, 8]);
}

fn big_house_1(c: &mut Canvas, v: Village) {
    let post = &K.log;
    let ridge = Some(&K.planks);
    c.solid(&S.dirt, [1, 0, 1], [5, 0, 9]);
    c.solid(&S.cobble, [2, 0, 2], [4, 0, 8]);
    c.place(&S.cobble, 1, 0, 5);
    c.place(&S.path, 0, 0, 5);

    c.walls(&S.cobble, post, [1, 1, 1], [5, 3, 9]);
    c.walls(&K.planks, post, [1, 5, 1], [5, 7, 9]);
    c.posts(post, &[5], &[5], [1, 7]);
    c.posts(post, &[1], &[5], [5, 7]);
    c.walls(post, post, [1, 4, 1], [5, 4, 9]);
    c.solid(&K.log_z, [1, 4, 2], [1, 4, 8]);
    c.solid(&K.log_x, [2, 4, 9], [4, 4, 9]);
    c.boxes(&K.planks, &[([2, 4, 2], [3, 4, 8]), ([4, 4, 7], [4, 4, 8])]);
    c.place(&K.planks, 4, 4, 2);

    c.stepped_gable(Gable::new(Z, [0, 6], [0, 10], 7, 4), &K.planks, None, ridge);
    c.fill(&K.planks, 1..=5, 8, [1, 9]);
    c.fill(&K.planks, 2..=4, 9, [1, 9]);
    c.fill(&S.pane, 3, [2, 6], [1, 9]);
    c.fill(&S.pane, [1, 5], [2, 6], [3, 7]);

    c.door(OAK_DOOR, [1, 1, 5], East, Hinge::Left);
    c.entrance([0, 1, 5], EMPTY, NOTHING);

    for step in 0..4 {
        c.place(&stairs(COBBLE_STAIRS, North), 4, 1 + step, 6 - step);
    }
    c.solid(&S.cobble, [4, 1, 4], [4, 1, 5]);
    c.place(&S.cobble, 4, 2, 4);
    c.solid(&S.cobble, [4, 3, 2], [4, 3, 3]);

    c.bed(WHITE_BED, [3, 1, 2], East);
    c.bed(WHITE_BED, [2, 1, 7], South);
    c.bed(WHITE_BED, [2, 5, 3], North);
    c.bed(WHITE_BED, [2, 5, 7], South);
    chest(c, [4, 5, 8], North, "village_plains_house");
    c.place(&wall_torch(South), 3, 3, 2);
    c.place(&S.wall_torch, 3, 3, 8);
    c.fill(&S.wall_torch, 3, 7, [2, 8]);
    c.place(&S.wall_torch, 4, 6, 5);
    villagers(c, v, COBBLE, &[[3, 0, 3], [3, 0, 8]]);
    villagers(c, v, PLANKS, &[[3, 4, 2], [3, 4, 8]]);
    open(c, 0..=0, &[]);
}

fn plains_butcher_shop_1(c: &mut Canvas) {
    let (wall, post) = (&S.cobble, &K.log);
    c.walls(wall, post, [1, 0, 1], [9, 1, 6]);
    c.walls(post, post, [1, 2, 1], [9, 2, 6]);
    c.walls(&K.planks, post, [1, 3, 1], [9, 3, 6]);
    c.posts(post, &[5], &[1], [0, 2]);
    c.fill(wall, [1, 9], 3..=4, 2..=5);
    c.fill(wall, [1, 9], 5, 3..=4);
    c.solid(&K.planks, [2, 0, 2], [8, 0, 5]);
    c.solid(&smooth_slab("double"), [6, 0, 2], [8, 0, 4]);
    roof(c, Gable::new(X, [0, 7], [0, 10], 3, 4), BOARDED);
    c.fill(&K.planks, 2..=8, 4, [2, 5]);
    c.fill(&K.planks, 2..=8, 5, [3, 4]);
    c.fill(&S.pane, [3, 7], 2, 1);
    c.solid(&S.pane, [9, 2, 3], [9, 2, 4]);
    c.solid(&K.log_x, [2, 2, 6], [6, 2, 6]);
    c.solid(&S.pane, [3, 2, 6], [4, 2, 6]);
    c.place(wall, 8, 2, 6);
    c.door(OAK_DOOR, [1, 1, 3], East, Hinge::Left);
    c.door(OAK_DOOR, [1, 1, 4], East, Hinge::Right);
    c.door(OAK_DOOR, [7, 1, 6], North, Hinge::Left);
    entrance(c, LIVING, [0, 0, 3], OAK_STEP);
    c.place(&stairs(OAK_STAIRS, East), 0, 0, 4);

    c.place(&stairs(OAK_STAIRS, West), 2, 1, 2);
    table(c, [3, 1, 2]);
    c.place(&stairs(OAK_STAIRS, East), 4, 1, 2);
    c.solid(&smooth_slab("double"), [7, 1, 2], [7, 1, 3]);
    c.place(&K.planks, 2, 1, 5);
    c.place(&block(Block::PottedDandelion.as_static_str()), 2, 2, 5);
    c.furnace([8, 1, 5], "smoker", West);
    c.solid(&low_wall(&[East, South]), [8, 2, 5], [8, 3, 5]);
    c.place(&low_wall(&[East, South, West]), 8, 4, 5);
    chimney(c, [8, 5], [5, 7]);
    c.place(&S.cobble_wall, 8, 6, 5);
    c.each(&S.wall_torch, &[[3, 3, 2], [7, 3, 5], [5, 2, 0]]);
    c.fill(&S.wall_torch, 0, 2, [2, 5]);

    c.solid(&GROUND, [4, 0, 7], [8, 0, 10]);
    c.place(wall, 7, 0, 7);
    c.fill(post, [3, 9], 0, 7..=11);
    c.fill(&K.fence, [3, 9], 1, 7..=11);
    c.fill(&S.torch, [3, 9], 2, 11);
    c.solid(post, [4, 0, 11], [8, 0, 11]);
    c.solid(&K.fence, [4, 1, 11], [8, 1, 11]);
    c.solid(&K.hay, [5, 1, 7], [6, 1, 7]);
    c.spot([6, 0, 9], BUTCHER_ANIMALS, DIRT);
}

fn butcher_shop_2(c: &mut Canvas, v: Village) {
    let (wall, post) = (&S.cobble, &K.log);
    c.solid(&S.dirt, [1, 0, 1], [8, 0, 5]);
    c.runs(wall, 0, &[[2, 3, 2], [6, 7, 2], [1, 2, 3], [6, 7, 3]]);
    c.runs(wall, 0, &[[2, 2, 4], [4, 4, 4], [6, 7, 4]]);
    c.runs(&smooth_slab("top"), 0, &[[3, 5, 3], [3, 3, 4], [5, 5, 4]]);
    c.place(wall, 8, 0, 3);
    c.place(&S.path, 0, 0, 3);
    c.solid(&S.grass, [9, 0, 1], [14, 0, 5]);

    c.walls(wall, post, [1, 1, 1], [8, 3, 5]);
    c.walls(&K.log_x, post, [1, 4, 1], [8, 4, 5]);
    c.walls(wall, post, [4, 5, 1], [8, 7, 5]);
    c.walls(&K.log_x, post, [4, 6, 1], [8, 6, 5]);
    c.walls(&K.log_x, post, [4, 8, 1], [8, 8, 5]);
    c.fill(&K.log_z, [1, 8], 4, 2..=4);
    c.fill(&K.log_z, [4, 8], [6, 8], 2..=4);
    c.fill(wall, [4, 8], 9, 2..=4);
    c.fill(&S.pane, [4, 8], 6, 3);
    c.fill(&K.log_x, 2..=7, 2, [1, 5]);
    c.fill(&S.pane, [3, 6], 2, [1, 5]);
    c.fill(&S.pane, 6, 6, [1, 5]);

    roof(c, Gable::new(X, [0, 6], [0, 9], 4, 1), boards(true, false));
    roof(c, Gable::new(X, [1, 5], [0, 3], 5, 1), BARE);
    c.solid(&K.planks, [0, 5, 2], [3, 5, 4]);
    c.solid(&stairs(OAK_STAIRS, West), [9, 4, 1], [9, 4, 5]);
    ridged_roof(c, Gable::new(X, [0, 6], [3, 9], 8, 3), BOARDED);

    c.door(OAK_DOOR, [1, 1, 3], East, Hinge::Right);
    c.door(OAK_DOOR, [8, 1, 3], West, Hinge::Right);
    entrance(c, v, [0, 1, 3], NOTHING);

    for step in 0..4 {
        c.place(&stairs(OAK_STAIRS, East), 4 + step, 1 + step, 2);
    }
    c.place(&K.planks, 5, 1, 2);
    c.solid(&stairs(OAK_STAIRS, North), [6, 1, 2], [7, 1, 2]);
    c.place(&K.planks, 7, 3, 2);
    c.solid(&K.planks, [5, 4, 3], [7, 4, 4]);
    c.solid(&top_stairs(OAK_STAIRS, East), [4, 4, 3], [4, 4, 4]);
    c.place(&smooth_slab("double"), 4, 1, 4);
    c.furnace([5, 5, 4], "smoker", East);
    c.solid(&low_wall(&[South, West]), [5, 6, 4], [5, 9, 4]);
    chimney(c, [5, 4], [10, 11]);
    c.fill(&S.wall_torch, [5, 7], 7, 3);
    c.fill(&S.wall_torch, [0, 2], 3, 3);
    c.place(&wall_torch(West), 7, 3, 3);
    c.fill(&K.fence, 9..=14, 1, [1, 5]);
    c.solid(&K.fence, [14, 1, 2], [14, 1, 4]);
    if v.lit() {
        c.each(&S.wall_torch, &[[9, 3, 3], [3, 8, 3]]);
        c.fill(&S.torch, 14, 2, [1, 5]);
    } else {
        c.place(&S.grass, 4, 0, 2);
    }
    c.spot([12, 0, 3], BUTCHER_ANIMALS, v.turf());
    c.spot(if v.zombie { [11, 0, 6] } else { [0, 0, 0] }, CATS, GRASS);
    c.spot([13, 0, 6], CATS, GRASS);
    decorations(c, v, DIRT, &[[5, 6]]);
    decorations(c, v, v.turf(), &[[14, 0]]);
    if !v.zombie {
        open(c, 0..=0, &[]);
    }
}

fn stable_1(c: &mut Canvas, v: Village) {
    let wall = &S.cobble;
    c.boxes(&GROUND, &[([0, 0, 0], [8, 0, 5]), ([2, 0, 6], [6, 0, 14])]);
    c.solid(&S.grass, [3, 0, 13], [if v.zombie { 5 } else { 3 }, 0, 13]);
    c.solid(&S.path, [0, 0, 7], [1, 0, 7]);

    c.solid(&K.fence, [0, 1, 0], [8, 1, 0]);
    c.fill(&K.fence, [0, 8], 1, 1..=5);
    c.fill(&K.fence, [1, 7], 1, 5);

    c.fill(wall, [2, 6], 1..=4, 5..=14);
    c.boxes(wall, &[([3, 1, 14], [5, 5, 14]), ([3, 4, 5], [5, 5, 5])]);
    for (x, facing) in [(3, West), (5, East)] {
        c.place(&top_stairs(COBBLE_STAIRS, facing), x, 3, 5);
    }
    for (x, panes) in [(2, [10, 12]), (6, [9, 11])] {
        c.solid(&K.log_z, [x, 2, panes[0] - 1], [x, 2, panes[1] + 1]);
        c.fill(&S.pane, x, 2, panes);
    }
    c.window(&S.pane, Some(&K.log_x), [4, 2, 14], X);
    ridged_roof(c, Gable::new(Z, [1, 7], [4, 15], 4, 3), BOARDED);
    c.door(OAK_DOOR, [2, 1, 7], East, Hinge::Right);
    entrance(c, v, [0, 1, 8], NOTHING);

    c.solid(&K.log_x, [3, 1, 12], [5, 1, 12]);
    c.solid(&S.water, [3, 1, 13], [5, 1, 13]);
    c.fill(&K.hay, 3, 1, [3, 8, 9]);
    c.each(&K.hay, &[[5, 1, 5], [3, 2, 8]]);
    c.fill(&S.wall_torch, 4, 4, [6, 13]);
    c.place(&S.wall_torch, 4, 5, 15);
    if v.lit() {
        c.fill(&S.torch, [0, 8], 2, [0, 5]);
        c.place(&S.wall_torch, 4, 4, 4);
        c.fill(&S.wall_torch, [3, 5], 4, 10);
    }

    let edge = if v.zombie { 8 } else { 7 };
    spots(c, ANIMALS, v.turf(), &[[2, 0, 2], [4, 0, 10]]);
    spots(c, CATS, GRASS, &[[edge, 0, edge], [edge, 0, 14]]);
    decorations(c, v, v.turf(), &[[6, 1], [0, 11], [0, 15]]);
    if !v.zombie {
        open(c, 0..=0, &[]);
    }
}

fn plains_stable_2(c: &mut Canvas) {
    let (wall, post) = (&K.terracotta, &K.log);
    c.solid(&GROUND, [1, 0, 0], [5, 0, 15]);
    c.solid(&S.dirt, [1, 0, 14], [5, 0, 14]);
    c.fill(&S.grass, [2, 4], 0, 14);
    c.place(&S.dirt, 0, 0, 9);
    c.place(&S.path, 0, 0, 10);

    c.solid(&K.fence, [1, 1, 0], [5, 1, 0]);
    c.fill(&K.fence, [1, 5], 1, 1..=4);
    c.fill(&K.fence, [1, 5], 2..=3, 2);
    c.fill(wall, [1, 5], 1..=3, 5..=15);
    c.posts(post, &[1, 5], &[5, 8, 12, 15], [1, 3]);
    c.solid(wall, [2, 1, 15], [4, 4, 15]);
    c.place(&S.pane, 3, 2, 15);
    c.solid(&K.log_x, [2, 3, 5], [4, 3, 5]);
    c.solid(wall, [2, 4, 5], [4, 4, 5]);
    c.place(&S.pane, 5, 2, 10);

    ridged_roof(c, Gable::new(Z, [1, 5], [4, 16], 4, 2), BOARDED);
    for (eave, inner, facing) in [(0, 1, East), (6, 5, West)] {
        let under = top_stairs(OAK_STAIRS, facing.opposite());
        c.fill(&stairs(OAK_STAIRS, facing), eave, 3, [4..=8, 12..=16]);
        c.place(&top_stairs(OAK_STAIRS, North), eave, 3, 9);
        c.place(&top_stairs(OAK_STAIRS, South), eave, 3, 11);
        c.solid(&stairs(OAK_STAIRS, facing), [eave, 4, 9], [eave, 4, 11]);
        c.solid(&K.planks, [inner, 4, 9], [inner, 4, 11]);
        c.fill(&under, inner, 3, [4, 16]);
        c.solid(&stairs(OAK_STAIRS, facing), [inner, 4, 2], [inner, 4, 3]);
    }
    c.fill(&K.slab_double, [2, 4], 4, 2..=4);
    c.solid(&K.slab_top, [3, 4, 2], [3, 4, 4]);
    c.solid(&K.slab, [3, 5, 2], [3, 5, 3]);

    c.door(OAK_DOOR, [1, 1, 10], East, Hinge::Left);
    entrance(c, LIVING, [0, 1, 9], NOTHING);
    c.solid(&K.log_x, [2, 1, 13], [4, 1, 13]);
    c.solid(&S.water, [2, 1, 14], [4, 1, 14]);
    c.place(&K.hay, 2, 1, 6);
    c.fill(&K.hay, 4, 1, [7, 8]);
    c.place(&K.hay, 4, 2, 8);
    c.place(&log(Block::HayBlock.as_static_str(), Z), 4, 1, 9);
    c.fill(&S.wall_torch, 3, 3, [4, 6]);
    c.place(&S.wall_torch, 2, 3, 10);

    spots(c, ANIMALS, GRASS, &[[3, 0, 2], [3, 0, 4]]);
    spots(c, CATS, GRASS, &[[0, 0, 3], [6, 0, 1]]);
}

fn fletcher_house_1(c: &mut Canvas, v: Village) {
    let (wall, post) = (&S.cobble, &K.log);
    let middle = 5;

    c.solid(&K.planks, [2, 0, middle], [6, 0, middle]);
    c.place(&S.dirt, 7, 0, middle);
    for reach in 1..=3 {
        let bay = if reach == 3 { wall } else { &K.planks };
        let sides = [middle - reach, middle + reach];
        c.fill(&S.dirt, [1 + reach, 7], 0, sides);
        c.fill(&K.planks, 2 + reach..=6, 0, sides);
        c.fill(bay, 1 + reach, 1..=4, sides);
    }
    c.solid(&K.planks, [2, 3, middle], [2, 5, middle]);
    c.fill(&S.dirt, 4..=7, 0, [1, 9]);
    c.fill(wall, 5..=6, [1, 2, 3, 5], [1, 9]);
    c.posts(post, &[4, 7], &[1, 9], [1, 4]);
    c.fill(&K.log_x, 5..=6, 4, [1, 9]);
    c.fill(&S.pane, 5..=6, 2, [1, 9]);
    c.solid(wall, [7, 1, 2], [7, 3, 8]);
    c.solid(&K.log_z, [7, 4, 2], [7, 4, 8]);
    c.solid(wall, [7, 5, 4], [7, 5, 6]);
    c.fill(&S.pane, 7, 2, [3, 5, 7]);
    c.solid(&S.path, [0, 0, middle], [1, 0, middle]);
    c.place(&S.dirt, 0, 0, 6);
    c.fill(&S.grass, 0, 0, [4, 8, 9, 10]);
    c.fill(&S.grass, 1, 0, [8, 10]);

    ridged_roof(c, Gable::new(X, [3, 7], [3, 8], 5, 2), boards(false, true));
    roof(c, Gable::new(Z, [3, 8], [0, 3], 4, 3), boards(true, false));
    roof(c, Gable::new(Z, [3, 8], [7, 10], 4, 3), boards(false, true));
    for (near, far, facing) in [(3, 2, South), (7, 8, North)] {
        c.place(&K.planks, 3, 4, near);
        c.each(&stairs(OAK_STAIRS, facing), &[[3, 4, far], [2, 4, near]]);
        c.place(&top_stairs(OAK_STAIRS, facing.opposite()), 8, 4, near);
    }
    for (z, facing) in [(4, South), (6, North)] {
        c.place(&K.planks, 3, 5, z);
        c.place(&stairs(OAK_STAIRS, facing), 2, 5, z);
        c.place(&stairs(OAK_STAIRS, East), 5, 6, z);
    }

    c.door(OAK_DOOR, [2, 1, middle], East, Hinge::Left);
    c.fill(&K.fence, 0, 1..=3, [4, 6]);
    c.fill(&K.slab, 0..=1, 4, [4, 6]);
    c.place(&K.yellow_wool, 0, 4, middle);
    c.place(&K.white_wool, 1, 4, middle);
    c.entrance([0, 1, 6], EMPTY, FENCE);

    c.place(&block(Block::FletchingTable.as_static_str()), 6, 1, 2);
    c.solid(
        &block(Block::YellowCarpet.as_static_str()),
        [5, 1, 4],
        [5, 1, 6],
    );
    c.solid(&K.planks, [5, 1, 8], [6, 1, 8]);
    c.place(&block(Block::PottedDandelion.as_static_str()), 6, 2, 8);
    c.scatter(&S.short_grass, 1, &[[0, 8], [1, 8], [1, 10]]);
    c.scatter(&S.tall_grass, 1, &[[0, 9], [0, 10]]);
    c.fill(&S.wall_torch, [1, 3], 3, middle);
    if v.lit() {
        c.fill(&S.wall_torch, 8, 2, [4, 6]);
        c.fill(&S.wall_torch, 6, 4, [4, 6]);
    }
    decorations(c, v, DIRT, &[[1, 1]]);
    open(c, 0..=0, &[]);
}

fn shepherds_house_1(c: &mut Canvas, v: Village) {
    let post = &K.log;
    let middle = 6;

    c.solid(&S.dirt, [2, 0, 1], [5, 0, 11]);
    c.solid(&K.planks, [3, 0, 2], [4, 0, 10]);
    c.solid(&K.planks, [2, 0, middle], [5, 0, middle]);
    checker(c, [&K.yellow_wool, &K.white_wool], 0, [3, 3], [4, 5]);
    checker(c, [&K.white_wool, &K.yellow_wool], 0, [3, 7], [4, 9]);
    c.solid(&S.grass, [6, 0, 3], [8, 0, 9]);
    c.solid(&S.path, [0, 0, middle], [1, 0, middle]);
    c.place(&S.dirt, 0, 0, 4);
    c.fill(&S.grass, 0, 0, [5, 7]);

    c.walls(&K.planks, post, [2, 1, 1], [5, 3, 11]);
    c.fill(&K.log_z, [2, 5], 2, 2..=10);
    c.fill(&S.pane, [2, 5], 2, [3, 9]);
    c.fill(&K.planks, [2, 5], 4, 5..=7);
    c.fill(&K.planks, 2, 2, [5, 7]);

    roof(c, Gable::new(Z, [1, 6], [0, 3], 3, 2), boards(true, false));
    roof(c, Gable::new(Z, [1, 6], [9, 12], 3, 2), boards(false, true));
    ridged_roof(c, Gable::new(X, [4, 8], [2, 6], 4, 2), boards(false, true));
    c.fill(&K.planks, 3..=4, 4, [0..=4, 8..=12]);
    for (z, facing) in [(4, North), (8, South)] {
        c.place(&stairs(OAK_STAIRS, West), 5, 4, z);
        c.place(&stairs(OAK_STAIRS, facing), 1, 3, z);
        c.place(&top_stairs(OAK_STAIRS, facing), 6, 3, z);
    }

    c.door(OAK_DOOR, [2, 1, middle], East, Hinge::Right);
    c.door(OAK_DOOR, [5, 1, middle], West, Hinge::Left);
    c.fill(&K.fence, 0, 1..=3, [5, 7]);
    c.fill(&S.wall_torch, 1, 2, [5, 7]);
    c.fill(&K.slab, 0..=1, 4, [5, 7]);
    c.place(&K.white_wool, 0, 4, middle);
    c.place(&K.yellow_wool, 1, 4, middle);
    c.entrance([0, 1, 4], EMPTY, NOTHING);
    c.fill(&K.fence, 6..=8, 1, [3, 9]);
    c.solid(&K.fence, [8, 1, 4], [8, 1, 8]);

    c.solid(&block("minecraft:loom[facing=south]"), [3, 1, 2], [4, 1, 2]);
    c.solid(&stairs(OAK_STAIRS, South), [3, 1, 10], [4, 1, 10]);
    c.fill(&wall_torch(South), 3..=4, 3, 2);
    c.fill(&wall_torch(North), 3..=4, 3, 10);
    c.place(&S.wall_torch, 6, 3, middle);
    c.spot([7, 0, 5], SHEEP, DIRT);
    decorations(c, v, DIRT, &[[0, 0], [8, 1]]);
    open(c, 0..=0, &[]);
    c.void([3, 1, 12], [3, 1, 12]);
}

fn plains_tool_smith_1(c: &mut Canvas) {
    let (wall, post) = (&K.planks, &K.log);

    c.solid(&S.cobble, [4, 0, 1], [7, 0, 10]);
    c.solid(&S.cobble, [2, 0, 7], [3, 0, 10]);
    c.solid(&stairs(COBBLE_STAIRS, East), [1, 0, 8], [1, 0, 9]);
    c.boxes(wall, &[([5, 1, 1], [6, 3, 1]), ([4, 1, 2], [4, 3, 6])]);
    c.boxes(wall, &[([7, 1, 2], [7, 3, 9]), ([3, 1, 7], [3, 3, 7])]);
    c.boxes(wall, &[([3, 1, 10], [6, 3, 10]), ([2, 3, 8], [2, 3, 9])]);
    for (x, z) in [(4, 1), (7, 1), (2, 7), (4, 7), (2, 10), (7, 10)] {
        c.solid(post, [x, 0, z], [x, 3, z]);
    }
    c.boxes(&K.log_z, &[([4, 2, 3], [4, 2, 5]), ([7, 2, 3], [7, 2, 8])]);
    c.scatter(&S.pane, 2, &[[4, 4], [7, 4], [7, 7]]);

    roof(c, Gable::new(Z, [3, 8], [0, 11], 3, 3), BOARDED.from(1, 0));
    c.solid(&stairs(OAK_STAIRS, East), [3, 3, 0], [3, 3, 5]);
    c.fill(&top_stairs(OAK_STAIRS, West), 4, 3, [0, 11]);
    roof(c, Gable::new(X, [6, 11], [1, 3], 3, 3), boards(true, false));
    c.each(&stairs(OAK_STAIRS, South), &[[4, 4, 7], [4, 5, 8]]);
    c.solid(&stairs(OAK_STAIRS, North), [4, 5, 9], [5, 5, 9]);
    c.solid(wall, [2, 4, 8], [3, 4, 9]);
    c.fill(wall, 5..=6, 4, [1, 10]);

    c.door(OAK_DOOR, [2, 1, 8], East, Hinge::Left);
    c.door(OAK_DOOR, [2, 1, 9], East, Hinge::Right);
    c.entrance([0, 0, 8], EMPTY, NOTHING);
    c.place(wall, 5, 1, 2);
    c.place(&block(Block::SmithingTable.as_static_str()), 6, 1, 2);
    c.fill(&S.wall_torch, [4, 7], 2, 0);
    c.fill(&S.wall_torch, 1, 2, [7, 10]);
    c.fill(&S.wall_torch, 6, 3, [4, 7]);
    c.place(&S.wall_torch, 4, 4, 9);
    c.place(&wall_torch(East), 4, 4, 8);
}

fn plains_tannery_1(c: &mut Canvas) {
    let (wall, post) = (&S.cobble, &K.log);
    let cauldron = block("minecraft:water_cauldron[level=3]");

    c.solid(wall, [3, 0, 1], [6, 0, 8]);
    c.fill(wall, 3..=6, 1..=3, [1, 8]);
    c.boxes(wall, &[([3, 1, 2], [3, 3, 6]), ([6, 1, 2], [6, 3, 7])]);
    c.solid(&K.planks, [4, 0, 2], [5, 0, 7]);
    c.place(&K.planks, 3, 0, 7);
    c.place(wall, 2, 0, 7);
    c.place(&stairs(COBBLE_STAIRS, East), 1, 0, 7);
    for (x, z) in [(3, 1), (6, 1), (2, 6), (2, 8), (6, 8)] {
        c.solid(post, [x, 0, z], [x, 3, z]);
    }
    c.place(wall, 2, 3, 7);
    c.fill(&K.fence, 0, 0..=2, [1, 5]);
    for z in [6, 8] {
        c.torch_post(&S.cobble_wall, [1, 0, z], 1);
    }
    c.scatter(&S.pane, 2, &[[6, 3], [6, 6], [4, 8]]);

    roof(c, Gable::new(Z, [2, 7], [0, 4], 3, 3), boards(true, false));
    ridged_roof(c, Gable::new(X, [5, 9], [1, 2], 3, 2), boards(true, false));
    ridged_roof(c, Gable::new(Z, [2, 6], [8, 9], 4, 2), boards(false, true));
    c.solid(&stairs(OAK_STAIRS, West), [7, 3, 5], [7, 3, 9]);
    c.solid(&stairs(OAK_STAIRS, West), [6, 4, 5], [6, 4, 7]);
    c.solid(&stairs(OAK_STAIRS, West), [5, 5, 5], [5, 5, 7]);
    c.solid(&K.planks, [4, 5, 5], [4, 5, 7]);
    c.solid(&stairs(OAK_STAIRS, East), [3, 5, 5], [3, 5, 7]);
    c.solid(&K.planks, [3, 4, 5], [3, 4, 7]);
    c.place(&top_stairs(OAK_STAIRS, West), 2, 3, 9);
    c.place(&top_stairs(OAK_STAIRS, East), 6, 3, 9);
    c.boxes(wall, &[([3, 4, 8], [5, 4, 8]), ([4, 4, 1], [5, 4, 1])]);
    c.solid(&K.slab, [0, 3, 1], [1, 3, 4]);
    c.place(&K.slab, 0, 3, 5);

    c.door(OAK_DOOR, [2, 1, 7], East, Hinge::Left);
    entrance(c, LIVING, [0, 0, 7], NOTHING);
    c.solid(&cauldron, [2, 0, 2], [2, 0, 4]);
    c.place(&cauldron, 5, 1, 2);
    c.place(&block(Block::SmoothStone.as_static_str()), 4, 1, 2);
    chest(c, [5, 1, 7], North, "village_tannery");
    c.solid(&wall_torch(South), [4, 3, 2], [5, 3, 2]);
    c.place(&wall_torch(East), 3, 3, 7);
}

fn medium_house_1(c: &mut Canvas, v: Village) {
    let (wall, post) = (&S.cobble, &K.stripped);
    let (east, west) = (stairs(OAK_STAIRS, East), stairs(OAK_STAIRS, West));

    c.boxes(&S.dirt, &[([1, 0, 1], [6, 0, 2]), ([1, 0, 3], [11, 0, 9])]);
    c.place(&S.dirt, 0, 0, 4);
    c.place(&S.grass, 0, 0, 3);
    c.solid(&K.planks, [2, 1, 2], [5, 1, 8]);
    c.solid(&K.planks, [6, 1, 4], [10, 1, 8]);

    for level in 0..4 {
        c.solid(&east, [level, 4 + level, 0], [level, 4 + level, 10]);
    }
    c.solid(&K.planks, [1, 4, 0], [1, 4, 10]);
    c.solid(&K.planks, [2, 5, 0], [2, 5, 10]);
    c.solid(&K.planks, [3, 6, 0], [4, 6, 10]);
    for (y, [north, south]) in (4..).zip([[1, 10], [2, 10], [3, 9], [4, 8]]) {
        c.fill(&west, 11 - y, y, [0..=north, south..=10]);
    }
    c.solid(&K.planks, [6, 4, 0], [6, 4, 3]);
    c.each(&K.planks, &[[6, 4, 10], [7, 4, 2]]);
    c.solid(&K.planks, [5, 5, 0], [5, 5, 4]);
    c.solid(&K.planks, [5, 5, 8], [5, 5, 10]);
    c.boxes(&K.planks, &[([5, 6, 4], [5, 6, 5]), ([5, 6, 7], [5, 6, 8])]);
    c.solid(&K.planks, [4, 7, 5], [4, 7, 7]);

    for (y, [north, south]) in (4..).zip([[8, 8], [7, 6], [6, 6], [5, 5]]) {
        c.fill(&stairs(OAK_STAIRS, South), north..=12, y, y - 2);
        c.fill(&stairs(OAK_STAIRS, North), south..=12, y, 14 - y);
    }
    c.fill(&K.planks, 12, 4, [3, 9]);
    c.fill(wall, 3..=4, 5, [1, 9]);
    c.solid(&K.planks, [6, 5, 3], [6, 5, 4]);
    c.solid(&K.planks, [7, 5, 4], [12, 5, 4]);
    c.solid(&K.planks, [3, 5, 8], [12, 5, 8]);
    c.fill(&K.planks, 6..=12, 6, [5, 7]);
    c.fill(&K.log, 11, 5, [5, 7]);
    c.solid(&K.planks, [5, 7, 6], [12, 7, 6]);
    c.place(&S.pane, 11, 5, 6);
    c.place(wall, 11, 6, 6);

    c.boxes(wall, &[([2, 1, 1], [5, 4, 1]), ([7, 1, 3], [10, 4, 3])]);
    c.boxes(wall, &[([11, 1, 4], [11, 4, 8]), ([2, 1, 9], [10, 4, 9])]);
    c.boxes(wall, &[([1, 1, 2], [1, 2, 8]), ([6, 1, 2], [6, 2, 3])]);
    c.solid(&K.planks, [6, 3, 2], [6, 3, 3]);
    for (x, z) in [(1, 1), (6, 1), (11, 3), (1, 9), (11, 9)] {
        c.solid(post, [x, 1, z], [x, 4, z]);
    }
    for (x, z) in [(2, 1), (7, 3), (2, 9), (7, 9)] {
        c.fill(&K.log, [x, x + 3], 3, z);
        c.fill(&S.pane, [x + 1, x + 2], 3, z);
    }
    c.fill(&log(STRIPPED_LOG, Z), 1, 3, [2, 4, 8]);
    c.window(&S.pane, Some(&K.log), [1, 3, 6], Z);
    c.place(&log(STRIPPED_LOG, X), 6, 3, 9);

    c.door(OAK_DOOR, [1, 2, 3], East, Hinge::Left);
    c.place(&east, 0, 1, 3);
    c.entrance([0, 1, 4], EMPTY, NOTHING);
    for z in [5, 7] {
        c.bed(WHITE_BED, [9, 2, z], East);
    }
    c.place(&stairs(OAK_STAIRS, North), 2, 2, 5);
    c.place(&stairs(OAK_STAIRS, South), 2, 2, 7);
    for at in [[10, 2, 4], [2, 2, 6], [6, 2, 8], [10, 2, 8]] {
        table(c, at);
    }
    c.fill(&S.wall_torch, 0, 3, [2, 4]);
    c.fill(&S.wall_torch, [10, 12], 3, 6);
    c.each(&S.wall_torch, &[[2, 4, 3], [6, 4, 8]]);
    villagers(c, v, PLANKS, &[[6, 1, 6], [4, 1, 7]]);
    decorations(c, v, DIRT, &[[10, 0]]);
    open(c, 0..=0, &[]);
}

fn plains_weaponsmith_1(c: &mut Canvas) {
    let (wall, post) = (&S.cobble, &K.log);
    let lava = block("minecraft:lava[level=0]");
    let bars = settled("minecraft:iron_bars[waterlogged=false]");
    let grindstone = block("minecraft:grindstone[face=floor,facing=east]");
    let rim = smooth_slab("bottom");
    c.boxes(wall, &[([1, 0, 0], [7, 0, 9]), ([1, 4, 0], [7, 4, 9])]);
    c.posts(post, &[1, 7], &[6, 9], [0, 5]);
    c.fill(&K.fence, 1, 1..=3, [0, 4]);
    c.solid(wall, [7, 1, 0], [7, 3, 5]);
    c.fill(wall, 5..=6, [1, 3], 0);
    c.boxes(wall, &[([5, 1, 1], [5, 1, 2]), ([5, 3, 1], [6, 3, 2])]);
    c.solid(wall, [5, 1, 3], [6, 3, 3]);
    c.place(wall, 4, 1, 3);
    c.solid(wall, [4, 1, 4], [4, 3, 4]);
    c.solid(post, [4, 1, 5], [4, 3, 5]);
    c.window(&S.pane, Some(&K.log_z), [7, 2, 4], Z);
    c.solid(&lava, [6, 1, 1], [6, 1, 2]);
    c.solid(&bars, [5, 2, 0], [6, 2, 0]);
    for y in [2, 3] {
        c.furnace([4, y, 3], "furnace", West);
    }
    c.place(&grindstone, 2, 1, 1);
    chest(c, [6, 1, 4], South, "village_weaponsmith");
    c.solid(&rim, [1, 5, 0], [7, 5, 0]);
    c.fill(&rim, [1, 7], 5, 1..=4);
    c.solid(wall, [6, 5, 1], [6, 5, 2]);
    c.place(&S.cobble_wall, 6, 6, 1);
    c.place(&S.torch, 4, 5, 3);

    c.fill(&K.planks, [1, 7], [1, 2, 3, 5, 6], 7..=8);
    c.fill(&K.log_z, [1, 7], 4, 7..=8);
    c.solid(&K.planks, [2, 1, 9], [6, 3, 9]);
    c.solid(&K.log_x, [2, 4, 9], [6, 4, 9]);
    c.boxes(&K.planks, &[([2, 5, 9], [6, 5, 9]), ([3, 1, 6], [3, 3, 6])]);
    c.place(&K.planks, 2, 3, 6);
    c.fill(&S.pane, [3, 5], 2, 9);
    roof(c, Gable::new(X, [5, 10], [0, 8], 5, 3), BOARDED);
    c.door(OAK_DOOR, [2, 1, 6], South, Hinge::Right);
    c.fill(&stairs(COBBLE_STAIRS, East), 0, 0, [1, 2]);
    entrance(c, LIVING, [0, 0, 3], COBBLE_STEP);

    table(c, [5, 1, 7]);
    c.place(&stairs(OAK_STAIRS, East), 6, 1, 7);
    c.place(&stairs(OAK_STAIRS, South), 5, 1, 8);
    c.place(&K.planks, 6, 1, 8);
    c.each(&S.wall_torch, &[[8, 2, 1], [4, 2, 10]]);
    c.place(&wall_torch(North), 3, 2, 5);
    c.place(&wall_torch(West), 6, 2, 8);
}

fn plains_temple_4(c: &mut Canvas) {
    let stone = &S.cobble;
    let (yellow, white) = (&K.yellow_pane, &K.white_pane);
    let slab = block("minecraft:cobblestone_slab[type=bottom,waterlogged=false]");
    let ladder = block("minecraft:ladder[facing=north,waterlogged=false]");

    c.boxes(stone, &[([4, 0, 3], [9, 0, 3]), ([1, 5, 2], [8, 5, 4])]);
    c.solid(stone, [9, 1, 2], [9, 4, 4]);
    c.each(stone, &[[8, 1, 3], [1, 2, 3]]);
    for (x, lower) in [(1, 3), (9, 2)] {
        c.place(yellow, x, lower, 3);
        c.place(white, x, lower + 1, 3);
    }
    for (side, apse, aisle, out) in [(1, 0, 2, North), (5, 6, 4, South)] {
        c.fill(stone, [1..=2, 4..=8], 0..=4, side);
        c.fill(stone, [0, 3], 0, side);
        c.torch_post(&S.cobble_wall, [0, 1, side], 1);
        c.place(&stairs(COBBLE_STAIRS, out), 3, 1, side);
        c.place(stone, 3, 4, side);
        c.place(&stairs(COBBLE_STAIRS, out.opposite()), 1, 4, side);
        c.place(yellow, 7, 2, side);
        c.place(white, 7, 3, side);
        c.solid(stone, [2, 5, side], [4, 5, side]);

        c.solid(stone, [2, 0, apse], [4, 3, apse]);
        c.place(yellow, 3, 2, apse);
        c.place(white, 3, 3, apse);
        c.place(&stairs(COBBLE_STAIRS, East), 2, 4, apse);
        c.place(stone, 3, 4, apse);
        c.place(&stairs(COBBLE_STAIRS, West), 4, 4, apse);
        c.place(&slab, 3, 5, apse);

        c.solid(stone, [1, 0, aisle], [9, 0, aisle]);
        c.solid(stone, [1, 1, aisle], [1, 4, aisle]);
        c.solid(stone, [7, 1, aisle], [8, 1, aisle]);
        c.place(&stairs(COBBLE_STAIRS, East), 6, 1, aisle);
        c.place(&stairs(COBBLE_STAIRS, out), 8, 2, aisle);
        c.place(&S.wall_torch, 7, 4, aisle);
    }
    c.each(&stairs(COBBLE_STAIRS, East), &[[3, 0, 3], [7, 1, 3]]);
    c.each(&S.wall_torch, &[[2, 2, 3], [8, 4, 3]]);
    c.place(&S.torch, 7, 6, 3);

    c.fill(stone, 2..=4, [6, 7, 8, 10], [1, 5]);
    c.fill(stone, [1, 5], [6, 7, 8, 10], 2..=4);
    for (y, pane) in [(7, yellow), (8, white), (11, stone)] {
        c.scatter(pane, y, &[[3, 1], [3, 5], [1, 3], [5, 3]]);
    }
    for (z, facing, inside) in [(1, South, 2), (5, North, 4)] {
        c.fill(&top_stairs(COBBLE_STAIRS, facing), [1, 5], 8, z);
        c.place(&wall_torch(facing), 2, 7, inside);
    }
    c.solid(stone, [1, 9, 1], [5, 9, 5]);
    c.place(&S.torch, 3, 10, 3);
    c.solid(&ladder, [4, 1, 4], [4, 10, 4]);

    c.door(OAK_DOOR, [1, 0, 3], East, Hinge::Left);
    c.entrance([0, 0, 3], EMPTY, NOTHING);
    c.brewing_stand([4, 1, 2]);
}

fn plains_library_1(c: &mut Canvas) {
    let (wall, post) = (&K.planks, &K.log);
    let middle = 8;

    c.solid(&S.cobble, [3, 0, 1], [9, 0, 15]);
    c.solid(&S.cobble, [2, 0, 6], [2, 0, 10]);
    c.solid(&S.cobble, [1, 0, 7], [1, 0, 9]);
    c.solid(wall, [9, 1, 2], [9, 5, 14]);
    c.solid(&K.log_z, [9, 4, 2], [9, 4, 14]);
    c.boxes(wall, &[([3, 3, 6], [5, 3, 10]), ([2, 3, 7], [2, 3, 9])]);
    c.place(&S.cobble, 1, 3, middle);
    c.solid(wall, [3, 4, 6], [3, 6, 10]);
    c.solid(&K.log_z, [3, 7, 6], [3, 7, 10]);
    c.boxes(wall, &[([8, 6, 2], [8, 6, 14]), ([7, 7, 2], [7, 7, 14])]);
    c.place(&S.pane, 9, 2, middle);

    for (sign, out) in [(-1, North), (1, South)] {
        let at = |distance: i32| middle + sign * distance;
        let span = |near: i32, far: i32| at(near).min(at(far))..=at(near).max(at(far));
        let inward = out.opposite();
        let end = at(7);
        c.solid(wall, [4, 1, end], [8, 6, end]);
        c.solid(&K.log_x, [4, 4, end], [8, 4, end]);
        c.posts(post, &[3, 9], &[end], [0, 5]);
        c.window(&S.pane, Some(&K.log_x), [6, 2, end], X);
        c.solid(&K.log_x, [5, 7, end], [7, 7, end]);
        c.place(&S.wall_torch, 2, 2, end);

        c.fill(wall, 3, 1..=5, span(4, 6));
        c.fill(&K.log_z, 3, 4, span(4, 6));
        c.place(&S.pane, 3, 2, at(5));
        c.solid(post, [3, 0, at(3)], [3, 6, at(3)]);
        c.solid(&S.cobble, [2, 1, at(2)], [2, 3, at(2)]);
        c.solid(&S.cobble, [1, 1, at(1)], [1, 3, at(1)]);
        c.fill(&K.log_z, 9, 2, [at(1), at(3), at(5)]);
        c.place(&S.pane, 9, 2, at(4));

        c.fill(wall, 4, 6, span(3, 6));
        c.fill(wall, 5, 7, span(2, 6));
        c.place(wall, 4, 7, at(2));
        c.solid(wall, [3, 8, at(1)], [5, 8, at(1)]);
        c.place(&S.wall_torch, 6, 6, at(6));

        c.each(&K.fence, &[[2, 4, at(2)], [1, 4, at(1)]]);
        for step in 0..3 {
            c.place(&stairs(COBBLE_STAIRS, inward), 5, 1 + step, at(5 - step));
        }
        c.solid(&S.cobble, [5, 1, at(3)], [5, 2, at(3)]);
        c.place(&S.cobble, 5, 1, at(4));
        c.solid(&stairs(OAK_STAIRS, out), [7, 1, at(6)], [8, 1, at(6)]);
        c.fill(&stairs(OAK_STAIRS, East), 8, 1, span(4, 5));
        c.fill(&S.bookshelf, 8, 1, span(1, 3));
        c.place(&S.bookshelf, 8, 2, at(2));
        c.lectern([5, 1, at(2)], inward);
        c.each(&S.wall_torch, &[[10, 2, at(2)], [0, 2, at(1)]]);
        c.place(&S.wall_torch, 6, 3, at(2));
    }
    c.place(&K.fence, 1, 4, middle);
    c.place(wall, 3, 8, middle);

    let hall = |length: [i32; 2]| Gable::new(Z, [2, 10], length, 5, 4);
    ridged_roof(c, hall([0, 4]), boards(true, false));
    ridged_roof(c, hall([5, 11]), BARE.from(4, 0));
    ridged_roof(c, hall([12, 16]), boards(false, true));
    ridged_roof(c, Gable::new(X, [5, 11], [2, 4], 7, 3), boards(true, false));
    for (sign, out) in [(-1, North), (1, South)] {
        let at = |distance: i32| middle + sign * distance;
        c.place(&stairs(OAK_STAIRS, East), 5, 8, at(3));
        c.place(&stairs(OAK_STAIRS, out.opposite()), 5, 8, at(2));
        c.place(&stairs(OAK_STAIRS, out.opposite()), 5, 9, at(1));
    }
    c.place(wall, 5, 9, middle);
    c.solid(&stairs(OAK_STAIRS, West), [6, 9, 7], [6, 9, 9]);
    c.place(&stairs(OAK_STAIRS, East), 4, 7, 11);

    c.door(OAK_DOOR, [1, 1, middle], East, Hinge::Right);
    c.entrance([0, 0, middle], EMPTY, COBBLE_STEP);
    c.door(OAK_DOOR, [3, 4, 7], East, Hinge::Right);
    c.door(OAK_DOOR, [3, 4, 9], East, Hinge::Left);
    c.place(&stairs(OAK_STAIRS, West), 4, 4, middle);
}

fn meeting_point_4(c: &mut Canvas, v: Village) {
    let path = [
        0, 0, 1, 1, 1, 2, 7, 7, 2, 4, 4, 3, 7, 7, 3, 2, 3, 4, 5, 5, 4, 7, 7, 4, 0, 0, 5, 2, 2, 5,
        5, 6, 5, 8, 8, 5, 1, 5, 6, 0, 0, 7, 3, 3, 7, 5, 5, 7, 8, 8, 7, 1, 1, 8, 3, 3, 8, 6, 6, 8,
        2, 2, 9, 5, 5, 9, 0, 1, 10, 6, 6, 10, 8, 8, 10, 0, 0, 11, 2, 2, 11, 5, 5, 11, 1, 1, 12, 3,
        3, 12, 7, 7, 12, 6, 6, 13, 8, 8, 13, 3, 3, 14, 6, 6, 14, 7, 7, 15,
    ];
    let cobbles = [
        3, 3, 6, 3, 1, 4, 4, 4, 4, 5, 6, 6, 2, 7, 0, 8, 4, 8, 5, 8, 2, 10, 4, 10, 7, 10, 3, 11, 4,
        12, 6, 12,
    ];
    let tufts = [
        0, 0, 2, 0, 5, 0, 7, 0, 3, 1, 4, 1, 9, 1, 0, 3, 9, 3, 6, 4, 8, 4, 9, 4, 7, 5, 8, 6, 9, 6,
        9, 7, 7, 8, 8, 8, 8, 11, 0, 12, 9, 12, 0, 13, 1, 14, 4, 14, 9, 14, 2, 15, 3, 15, 4, 15, 9,
        15,
    ];
    c.solid(&GROUND, [0, 0, 0], [9, 0, 15]);
    c.runs(&S.path, 0, path.as_chunks().0);
    c.scatter(&S.cobble, 0, cobbles.as_chunks().0);
    c.place(&S.mossy, 1, 0, 13);
    c.scatter(&S.short_grass, 1, tufts.as_chunks().0);
    c.scatter(&S.tall_grass, 1, &[[8, 0], [6, 2], [9, 5]]);
    c.scatter(&S.tall_grass, 1, &[[0, 15], [1, 15]]);
    c.scatter(&K.dandelion, 1, &[[6, 7], [0, 14]]);

    market_stall(c, [2, 1, 1], (X, 4), [&K.yellow_wool, &K.white_wool], true);
    market_stall(c, [2, 1, 12], (X, 4), [&K.white_wool, &K.yellow_wool], true);
    c.place(&K.planks, 7, 1, 6);
    c.bell([7, 2, 6], "floor", East);
    c.entrance([0, 1, 8], EMPTY, NOTHING);
    decorations(c, v, DIRT, &[[1, 1], [8, 2], [7, 13]]);
    c.spot([7, 0, 9], TREES, DIRT);
}

fn meeting_point_5(c: &mut Canvas, v: Village) {
    let path = [
        0, 0, 1, 3, 3, 2, 6, 6, 2, 0, 0, 3, 2, 3, 3, 5, 5, 3, 1, 5, 4, 0, 2, 5, 4, 6, 5, 0, 0, 6,
        3, 4, 6, 0, 6, 7, 1, 1, 8, 3, 3, 8, 6, 7, 8, 0, 0, 9, 5, 6, 9, 7, 7, 10,
    ];
    let tufts = [
        0, 0, 6, 0, 9, 0, 1, 1, 5, 1, 6, 1, 7, 1, 8, 1, 0, 2, 5, 2, 8, 3, 9, 3, 9, 6, 7, 7, 9, 7,
        0, 8, 5, 8, 1, 9, 7, 9, 9, 9, 1, 10,
    ];
    c.solid(&GROUND, [0, 0, 0], [9, 0, 10]);
    c.runs(&S.path, 0, path.as_chunks().0);
    c.scatter(&S.short_grass, 1, tufts.as_chunks().0);
    c.scatter(&S.tall_grass, 1, &[[8, 5], [8, 7]]);

    for z in [0, 8] {
        market_stall(c, [2, 1, z], (X, 3), [&K.white_wool, &K.yellow_wool], false);
    }
    market_stall(c, [6, 1, 4], (Z, 3), [&K.yellow_wool, &K.white_wool], false);
    c.bell([7, 3, 5], "ceiling", East);
    if v.lit() {
        c.place(&wall_torch(South), 3, 4, 3);
        c.place(&wall_torch(North), 3, 4, 7);
        c.place(&wall_torch(West), 5, 4, 5);
        c.spot([2, 0, 6], CATS, PATH);
    } else {
        c.place(&S.path, 2, 0, 6);
    }
    entrance(c, v, [0, 1, 5], NOTHING);
    decorations(c, v, DIRT, &[[7, 2], [8, 8]]);
}

fn fountain_01(c: &mut Canvas, v: Village) {
    let falling = block("minecraft:water[level=8]");
    let spreading = block("minecraft:water[level=1]");
    c.solid(&S.path, [0, 0, 1], [8, 0, 7]);
    c.fill(&S.path, 1..=7, 0, [0, 8]);
    c.solid(&S.cobble, [2, 0, 2], [6, 0, 6]);
    c.walls(&S.cobble, &S.torch, [2, 1, 2], [6, 1, 6]);
    c.solid(&spreading, [3, 1, 3], [5, 1, 5]);
    c.solid(&S.cobble, [4, 1, 4], [4, 2, 4]);
    for (x, z) in [(4, 3), (3, 4), (5, 4), (4, 5)] {
        c.solid(&falling, [x, 1, z], [x, 2, z]);
        c.place(&spreading, x, 3, z);
    }
    c.place(&S.water, 4, 3, 4);
    c.bell([4, 2, 2], "floor", South);
    street_ends(c, v, &[[4, 0], [0, 4], [8, 4], [4, 8]]);
    spots(c, CATS, PATH, &[[1, 0, 2], [2, 0, 7]]);
    if !v.zombie {
        c.spot([0, 0, 6], IRON_GOLEM, PATH);
        villagers(c, v, PATH, &[[2, 0, 1], [7, 0, 2], [7, 0, 7]]);
    }
    let mut kept = vec![[0, 3, 4]];
    for y in 1..=3 {
        kept.extend([2, 3, 5, 6].map(|z| [0, y, z]));
    }
    open(c, 1..=c.size()[1] - 1, &kept);
}

fn meeting_point_1(c: &mut Canvas, v: Village) {
    c.boxes(&S.path, &[([1, 0, 1], [8, 0, 8]), ([0, 0, 4], [9, 0, 5])]);
    c.solid(&S.path, [4, 0, 0], [5, 0, 9]);
    c.boxes(&S.cobble, &[([3, 0, 3], [6, 0, 6]), ([2, 1, 2], [7, 1, 7])]);
    c.solid(&S.water, [4, 0, 4], [5, 1, 5]);
    c.walls(&S.cobble, &S.cobble, [3, 2, 3], [6, 2, 6]);
    c.posts(&K.fence, &[3, 6], &[3, 6], [3, 4]);
    c.solid(&S.cobble, [3, 5, 3], [6, 5, 6]);
    if v.lit() {
        c.posts(&S.torch, &[3, 6], &[3, 6], [6, 6]);
    }
    c.bell([3, 2, 7], "floor", North);
    let well =
        mcrs_minecraft_worldgen_feature::keys::template_pool::VILLAGE_COMMON_WELL_BOTTOMS.as_str();
    c.socket_facing([6, 0, 3], "down_south", BOTTOM, well, COBBLE);
    street_ends(c, v, &[[5, 0], [0, 4], [9, 5], [4, 9]]);
    spots(c, CATS, COBBLE, &[[2, 1, 2], [2, 1, 4], [5, 1, 7]]);
    if !v.zombie {
        c.spot([1, 0, 6], IRON_GOLEM, PATH);
        villagers(c, v, PATH, &[[1, 0, 1], [1, 0, 8]]);
    }
    open(c, 0..=6, &[]);
}

fn meeting_point_2(c: &mut Canvas, v: Village) {
    let path = [
        4, 6, 0, 1, 2, 1, 4, 5, 1, 1, 2, 2, 4, 7, 2, 0, 3, 3, 5, 7, 3, 0, 0, 4, 2, 2, 4, 4, 4, 4,
        6, 6, 4, 0, 3, 5, 5, 5, 5, 0, 0, 6, 3, 3, 6, 0, 4, 7, 0, 1, 8, 5, 5, 8, 0, 0, 9, 3, 4, 9,
        4, 4, 10, 0, 0, 11, 1, 2, 12, 4, 5, 12, 4, 6, 13, 5, 6, 14,
    ];
    let streets = v.pool("streets");
    c.solid(&GROUND, [0, 0, 0], [7, 0, 14]);
    c.runs(&S.path, 0, path.as_chunks().0);
    for z in [0, 12] {
        market_stall(c, [0, 1, z], (X, 4), [&K.yellow_wool, &K.white_wool], true);
    }
    market_stall(c, [4, 1, 5], (Z, 4), [&K.white_wool, &K.yellow_wool], true);
    c.place(&K.planks, 5, 1, 10);
    c.bell([5, 2, 10], "floor", East);
    c.scatter(&S.short_grass, 1, &[[6, 7], [7, 8], [3, 10]]);
    c.scatter(&S.short_grass, 1, &[[3, 11], [6, 12]]);
    street_ends(c, v, &[[5, 0], [0, 7], [5, 14]]);
    if v.zombie {
        c.socket_facing([7, 1, 3], "north_up", "minecraft:street", &streets, NOTHING);
        c.place(&S.grass, 1, 0, 4);
        c.scatter(&S.path, 0, &[[3, 4], [1, 6], [2, 8]]);
        c.scatter(&S.path, 0, &[[2, 10], [1, 11], [4, 11]]);
        for (z, floor) in [(5, PATH), (9, GRASS)] {
            c.socket_facing([2, 0, z], "east_up", BOTTOM, CATS, floor);
        }
    } else {
        street_ends(c, v, &[[7, 3]]);
        c.spot([1, 0, 4], IRON_GOLEM, GRASS);
        spots(c, CATS, PATH, &[[3, 0, 4], [1, 0, 11], [4, 0, 11]]);
        villagers(c, v, GRASS, &[[1, 0, 6]]);
        villagers(c, v, PATH, &[[2, 0, 8], [2, 0, 10]]);
    }
}

fn meeting_point_3(c: &mut Canvas, v: Village) {
    let path = [
        4, 5, 0, 7, 9, 0, 0, 0, 1, 2, 2, 1, 9, 9, 1, 7, 8, 2, 10, 10, 2, 1, 1, 3, 5, 5, 3, 9, 9, 3,
        0, 0, 4, 3, 3, 4, 9, 9, 4, 0, 1, 5, 5, 5, 5, 7, 7, 5, 10, 10, 5, 3, 3, 6, 8, 8, 6, 3, 3, 7,
        5, 6, 7, 9, 9, 7, 1, 1, 8, 5, 5, 8, 7, 8, 8, 10, 10, 8, 2, 2, 9, 5, 5, 9, 8, 8, 9, 10, 10,
        9, 3, 3, 10, 6, 6, 10,
    ];
    let cobbles = [
        3, 3, 1, 5, 5, 1, 8, 8, 1, 10, 10, 1, 2, 2, 2, 4, 5, 2, 4, 4, 3, 6, 7, 3, 8, 8, 4, 10, 10,
        4, 3, 3, 5, 8, 9, 5, 1, 1, 6, 7, 7, 6, 0, 0, 7, 8, 8, 7, 3, 3, 8, 6, 6, 8, 9, 9, 8, 1, 1,
        9, 4, 4, 9, 7, 7, 9, 5, 5, 10, 8, 8, 10,
    ];
    let tufts = [
        0, 0, 2, 0, 3, 0, 6, 2, 9, 2, 0, 3, 10, 3, 7, 4, 2, 5, 9, 6, 4, 7, 2, 8, 4, 8, 7, 10,
    ];
    let dirt = Patch::rectangle([4, 4], [6, 6]).removed(&[[5, 5], [6, 6]]);
    c.solid(&S.grass, [0, 0, 0], [10, 0, 10]);
    c.runs(&S.path, 0, path.as_chunks().0);
    c.runs(&S.cobble, 0, cobbles.as_chunks().0);
    c.patch(&S.dirt, 0, &dirt);
    c.scatter(&S.short_grass, 1, tufts.as_chunks().0);
    c.place(&K.dandelion, 7, 1, 1);

    let (trunk, crown) = ([5, 5], 7);
    c.solid(&stairs(COBBLE_STAIRS, South), [4, 1, 4], [6, 1, 4]);
    c.place(&stairs(COBBLE_STAIRS, East), 4, 1, 5);
    c.solid(&stairs(COBBLE_STAIRS, West), [6, 1, 5], [6, 1, 6]);
    c.solid(&stairs(COBBLE_STAIRS, North), [4, 1, 6], [5, 1, 6]);
    let canopy = [
        Patch::clipped_rectangle([3, 3], [7, 7], 1).removed(&[[6, 3], [4, 7], [6, 7]]),
        Patch::clipped_rectangle([3, 3], [7, 7], 1).removed(&[[5, 7]]),
        Patch::rectangle([4, 4], [6, 6]),
        Patch::clipped_rectangle([4, 4], [6, 6], 1),
    ];
    for (y, layer) in (5..).zip(canopy) {
        for [x, z] in layer.cells() {
            let distance = (x - trunk[0]).abs() + (z - trunk[1]).abs() + (y - crown).max(0);
            let leaves = block(&format!(
                "minecraft:oak_leaves[distance={distance},persistent=false,waterlogged=false]"
            ));
            c.place(&leaves, x, y, z);
        }
    }
    c.solid(&K.log, [trunk[0], 1, trunk[1]], [trunk[0], crown, trunk[1]]);
    for (x, z, facing) in [(5, 4, North), (4, 5, West), (6, 5, East), (5, 6, South)] {
        c.place(&wall_torch(facing), x, 3, z);
    }
    for (z, facing) in [(1, South), (9, North)] {
        c.posts(&K.fence, &[4, 6], &[z], [1, 4]);
        c.bell([5, 3, z], "ceiling", facing);
        c.place(&S.cobble, 5, 4, z);
    }
    street_ends(c, v, &[[0, 5], [10, 5], [5, 10]]);
    spots(c, CATS, COBBLE, &[[2, 0, 4], [3, 0, 3]]);
    if v.zombie {
        c.walls(&S.grass, &S.grass, [4, 0, 4], [6, 0, 6]);
        c.place(&S.grass, 2, 0, 7);
        c.scatter(&S.path, 0, &[[1, 2], [2, 6]]);
        open(c, 1..=8, &[]);
    } else {
        c.spot([2, 0, 7], IRON_GOLEM, GRASS);
        villagers(c, v, PATH, &[[1, 0, 2], [2, 0, 6]]);
        open(c, 1..=8, &[[2, 1, 7], [2, 2, 7]]);
    }
}

templates! {
    "plains" Plains;
    both {
        "houses/plains_animal_pen_3" [8, 6, 11] animal_pen_3;
        "houses/plains_big_house_1" [7, 11, 11] big_house_1;
        "houses/plains_butcher_shop_2" [15, 12, 7] butcher_shop_2;
        "houses/plains_fletcher_house_1" [9, 7, 11] fletcher_house_1;
        "houses/plains_medium_house_1" [13, 8, 11] medium_house_1;
        "houses/plains_medium_house_2" [7, 6, 13] medium_house_2;
        "houses/plains_meeting_point_4" [10, 7, 16] meeting_point_4;
        "houses/plains_meeting_point_5" [10, 6, 11] meeting_point_5;
        "houses/plains_shepherds_house_1" [9, 6, 13] shepherds_house_1;
        "houses/plains_small_house_1" [7, 7, 7] small_house_1;
        "houses/plains_small_house_2" [7, 7, 7] small_house_2;
        "houses/plains_small_house_3" [7, 7, 7] small_house_3;
        "houses/plains_small_house_4" [7, 7, 7] small_house_4;
        "houses/plains_small_house_5" [9, 11, 9] small_house_5;
        "houses/plains_small_house_6" [7, 7, 7] small_house_6;
        "houses/plains_small_house_7" [7, 7, 8] small_house_7;
        "houses/plains_small_house_8" [8, 9, 9] small_house_8;
        "houses/plains_stable_1" [9, 7, 16] stable_1;
        "streets/corner_01" [16, 2, 16] corner_01_of;
        "streets/corner_02" [16, 2, 16] corner_02_of;
        "streets/corner_03" [4, 2, 4] corner_03_of;
        "streets/crossroad_01" [16, 2, 16] crossroad_01_of;
        "streets/crossroad_02" [16, 2, 16] crossroad_02_of;
        "streets/crossroad_03" [16, 2, 16] crossroad_03_of;
        "streets/crossroad_04" [4, 2, 5] crossroad_04_of;
        "streets/crossroad_05" [5, 2, 5] crossroad_05_of;
        "streets/crossroad_06" [5, 2, 5] crossroad_06_of;
        "streets/straight_01" [16, 2, 16] straight_01_of;
        "streets/straight_02" [16, 2, 16] |c, v| straight_with_houses(c, v, 16, [8, 8]);
        "streets/straight_03" [13, 2, 11] |c, v| straight_with_houses(c, v, 11, [3, 7]);
        "streets/straight_04" [11, 2, 9] |c, v| straight_with_houses(c, v, 9, [4, 4]);
        "streets/straight_05" [20, 2, 17] |c, v| straight_with_houses(c, v, 17, [7, 10]);
        "streets/straight_06" [21, 2, 18] straight_06_of;
        "streets/turn_01" [18, 2, 8] turn_01_of;
        "town_centers/plains_meeting_point_1" [10, 7, 10] meeting_point_1;
        "town_centers/plains_meeting_point_2" [8, 5, 15] meeting_point_2;
        "town_centers/plains_meeting_point_3" [11, 9, 11] meeting_point_3;
    }
    single {
        "houses/plains_accessory_1" [3, 2, 5] plains_accessory_1;
        "houses/plains_animal_pen_1" [5, 8, 6] plains_animal_pen_1;
        "houses/plains_animal_pen_2" [7, 7, 11] plains_animal_pen_2;
        "houses/plains_armorer_house_1" [9, 8, 8] plains_armorer_house_1;
        "houses/plains_butcher_shop_1" [11, 8, 12] plains_butcher_shop_1;
        "houses/plains_cartographer_1" [10, 8, 7] plains_cartographer_1;
        "houses/plains_fisher_cottage_1" [11, 9, 10] plains_fisher_cottage_1;
        "houses/plains_large_farm_1" [13, 6, 9] plains_large_farm_1;
        "houses/plains_library_1" [11, 10, 17] plains_library_1;
        "houses/plains_library_2" [8, 10, 9] plains_library_2;
        "houses/plains_masons_house_1" [8, 7, 9] plains_masons_house_1;
        "houses/plains_small_farm_1" [7, 6, 9] plains_small_farm_1;
        "houses/plains_stable_2" [7, 6, 17] plains_stable_2;
        "houses/plains_tannery_1" [8, 7, 10] plains_tannery_1;
        "houses/plains_temple_3" [11, 7, 7] plains_temple_3;
        "houses/plains_temple_4" [10, 12, 7] plains_temple_4;
        "houses/plains_tool_smith_1" [9, 6, 12] plains_tool_smith_1;
        "houses/plains_weaponsmith_1" [9, 8, 11] plains_weaponsmith_1;
        "plains_lamp_1" [3, 4, 3] plains_lamp_1;
        "terminators/terminator_01" [2, 2, 3] terminator_01;
        "terminators/terminator_02" [1, 2, 1] terminator_02;
        "terminators/terminator_03" [3, 2, 3] terminator_03;
        "terminators/terminator_04" [4, 2, 4] terminator_04;
        "town_centers/plains_fountain_01" [9, 4, 9] |c| fountain_01(c, LIVING);
        "zombie/town_centers/plains_fountain_01" [9, 6, 9] |c| fountain_01(c, ZOMBIE);
    }
}

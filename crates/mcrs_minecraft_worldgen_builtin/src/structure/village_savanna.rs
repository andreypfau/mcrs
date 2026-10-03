use super::*;
use mcrs_minecraft_core::{Axis, Direction};
use mcrs_minecraft_worldgen_structure::blueprint::{Cell, Coords, Hinge, Patch};

const STAIRS: &str = "minecraft:acacia_stairs";
const DOOR: &str = "minecraft:acacia_door";
const ORANGE_BED: &str = "minecraft:orange_bed";

const STREET: &str = "minecraft:street";
const ENTRANCE: &str = "minecraft:building_entrance";
const PLANKS: &str = "minecraft:acacia_planks";
const LOG: &str = "minecraft:acacia_log";
const WOOD: &str = "minecraft:acacia_wood";
const STEP_SOUTH: &str =
    "minecraft:acacia_stairs[facing=south,half=bottom,shape=straight,waterlogged=false]";
const FENCE: &str =
    "minecraft:acacia_fence[east=false,north=false,south=false,waterlogged=false,west=false]";

const TREES: &str = "minecraft:village/savanna/trees";
const STEP_EAST: &str = "minecraft:acacia_stairs[facing=east]";

/// The end of a street that opens towards `side` from inside the box.
fn inner_street_end(c: &mut Canvas, v: Village, [x, z]: [i32; 2], side: Direction) {
    let orientation = format!("{}_up", side.name());
    c.socket_facing([x, 1, z], &orientation, STREET, &v.pool("streets"), NOTHING);
}

kit! {
    K;
    planks: block(PLANKS),
    log: log(LOG, Y),
    log_x: log(LOG, X),
    log_z: log(LOG, Z),
    wood: log(WOOD, Y),
    wood_x: log(WOOD, X),
    wood_z: log(WOOD, Z),
    cave_air: block("minecraft:cave_air"),
    fence: settled("minecraft:acacia_fence[waterlogged=false]"),
    dry_farmland: block("minecraft:farmland[moisture=0]"),
    plate: block("minecraft:acacia_pressure_plate[powered=false]"),
}

fn melon_stem(age: i32) -> Cell {
    block(&format!("minecraft:melon_stem[age={age}]"))
}

/// Farmland on the bottom layer over `runs`, sown with wheat.
fn field(c: &mut Canvas, runs: &[[i32; 3]]) {
    c.runs(&S.farmland, 0, runs);
    c.runs(&wheat(0), 1, runs);
}

fn corner_01_of(c: &mut Canvas, v: Village) {
    street(c, v, &[[7, 0], [0, 8]]);
    path(c, Z, 7, [0, 6]);
    c.rows(&S.path, 0, 7, &[[0, 8], [0, 7], [0, 2]]);
    houses(c, v, South, 8, [5, 5]);
    houses(c, v, East, 8, [6, 6]);
    decorations(c, v, GRASS, &[[10, 3], [3, 4], [13, 9], [8, 12], [3, 13]]);
}

fn corner_03_of(c: &mut Canvas, v: Village) {
    street(c, v, &[[3, 1], [1, 3]]);
    c.rows(&S.path, 0, 0, &[[2, 3], [1, 3], [0, 3], [0, 2]]);
}

fn crossroad_02_of(c: &mut Canvas, v: Village) {
    street(c, v, &[[4, 0], [0, 5], [6, 5], [4, 9]]);
    c.scatter(&S.air, 1, &[[0, 1], [1, 2], [5, 3], [2, 6]]);
    c.rows(
        &S.path,
        0,
        0,
        &[[3, 5], [3, 5], [3, 4], [3, 4], [0, 6], [0, 6]],
    );
    c.runs(&S.path, 0, &[[0, 1, 6], [4, 6, 6], [4, 5, 7]]);
    path(c, Z, 4, [8, 9]);
    decorations(c, v, GRASS, &[[1, 2], [3, 6]]);
}

fn crossroad_03_of(c: &mut Canvas, v: Village) {
    street(c, v, &[[2, 0], [0, 8], [15, 8], [11, 15]]);
    c.scatter(&S.air, 1, &[[4, 0], [0, 1], [1, 1], [4, 1], [1, 5]]);
    path(c, Z, 2, [0, 6]);
    path(c, X, 8, [0, 15]);
    path(c, Z, 11, [10, 15]);
    houses(c, v, North, 7, [10, 11]);
    decorations(c, v, GRASS, &[[2, 4], [8, 11], [3, 13]]);
}

fn crossroad_05_of(c: &mut Canvas, v: Village) {
    village_plains::crossroad_05_of(c, v);
    c.place(&S.air, 0, 1, 0);
}

fn crossroad_06_of(c: &mut Canvas, v: Village) {
    village_plains::crossroad_05_of(c, v);
    decorations(c, v, GRASS, &[[2, 2]]);
}

fn crossroad_07_of(c: &mut Canvas, v: Village) {
    let path = [
        12, 12, 0, 14, 14, 0, 11, 13, 1, 11, 12, 2, 11, 12, 3, 10, 11, 4, 10, 11, 5, 9, 14, 6, 9,
        13, 7, 9, 10, 8, 14, 14, 8, 9, 10, 9, 9, 10, 10, 8, 10, 11, 8, 9, 12, 7, 9, 13,
    ];
    let farmland = [
        10, 10, 1, 14, 14, 1, 9, 10, 2, 14, 14, 2, 9, 9, 3, 13, 14, 3, 9, 9, 4, 13, 14, 4, 12, 12,
        5, 14, 14, 5, 11, 11, 8, 13, 13, 8, 11, 12, 9, 14, 14, 9, 11, 14, 10, 12, 13, 11, 10, 12,
        12,
    ];
    let open = [
        13, 2, 8, 3, 10, 3, 8, 4, 12, 4, 7, 5, 8, 5, 9, 5, 13, 5, 7, 6, 12, 6, 13, 6, 7, 7, 8, 7,
        11, 7, 12, 7, 13, 7, 6, 8, 7, 8, 8, 8, 14, 8, 6, 9, 13, 9, 6, 10, 7, 10, 10, 10, 6, 11, 9,
        11, 10, 11, 11, 11, 7, 12, 9, 12, 6, 13, 9, 13,
    ];
    street(c, v, &[[13, 0], [14, 7], [8, 13]]);
    c.scatter(&S.air, 1, open.as_chunks().0);
    c.runs(&S.path, 0, path.as_chunks().0);
    c.each(&S.dirt, &[[13, 0, 0], [14, 0, 7]]);
    field(c, farmland.as_chunks().0);
    c.fill(&K.dry_farmland, 9, 0, 2..=4);
    c.scatter(&S.water, 0, &[[13, 2], [10, 3], [13, 5], [12, 8], [11, 11]]);
    c.scatter(&wheat(1), 1, &[[10, 1], [14, 2], [11, 8], [13, 8], [14, 9]]);
    c.scatter(&wheat(1), 1, &[[12, 11], [12, 12]]);
    c.scatter(
        &wheat(2),
        1,
        &[[14, 1], [14, 4], [12, 10], [13, 10], [11, 12]],
    );
    c.scatter(&melon_stem(0), 1, &[[9, 4], [12, 5]]);
    c.place(&melon_stem(1), 13, 1, 4);
    c.scatter(&melon_stem(2), 1, &[[10, 2], [11, 10]]);
    inner_street_end(c, v, [7, 13], West);
    houses(c, v, West, 9, [6, 6]);
    decorations(c, v, PATH, &[[7, 12]]);
    decorations(c, v, NOTHING, &[[12, 4], [13, 9]]);
}

fn split_01_of(c: &mut Canvas, v: Village) {
    street(c, v, &[[4, 0], [7, 3]]);
    inner_street_end(c, v, [1, 4], West);
    c.rows(
        &S.path,
        0,
        0,
        &[[3, 5], [3, 6], [2, 7], [1, 3], [1, 2], [1, 1]],
    );
    c.runs(&S.path, 0, &[[6, 7, 3], [7, 7, 4]]);
    decorations(c, v, GRASS, &[[4, 4]]);
}

fn split_02_of(c: &mut Canvas, v: Village) {
    street(c, v, &[[9, 0], [13, 4], [10, 8]]);
    c.rows(&S.path, 0, 0, &[[8, 10], [8, 10], [8, 9], [8, 9], [8, 10]]);
    c.rows(&S.path, 0, 5, &[[8, 13], [9, 12], [9, 11], [9, 11]]);
    c.runs(&S.path, 0, &[[13, 13, 3], [12, 13, 4]]);
    houses(c, v, West, 8, [4, 4]);
}

/// A street along the west edge, `length` long, with places for houses on
/// its east side over each of `sockets`.
fn straight(c: &mut Canvas, v: Village, length: i32, sockets: &[[i32; 2]]) {
    street(c, v, &[[1, 0], [1, length - 1]]);
    path(c, Z, 1, [0, length - 1]);
    for range in sockets {
        houses(c, v, East, 2, *range);
    }
}

fn straight_02_of(c: &mut Canvas, v: Village) {
    straight(c, v, 16, &[[2, 4], [8, 11]]);
    c.scatter(&S.air, 1, &[[3, 4], [4, 12]]);
}

fn straight_04_of(c: &mut Canvas, v: Village) {
    straight(c, v, 9, &[[4, 4]]);
    c.place(&S.air, 2, 1, 3);
}

fn straight_05_of(c: &mut Canvas, v: Village) {
    straight(c, v, 17, &[[7, 10]]);
    c.solid(&S.air, [7, 1, 0], [19, 1, 16]);
}

fn straight_06_of(c: &mut Canvas, v: Village) {
    let path = [
        6, 8, 0, 5, 7, 1, 5, 6, 2, 5, 6, 3, 4, 5, 4, 4, 5, 5, 3, 5, 6, 3, 4, 7, 3, 4, 8, 2, 4, 9,
        1, 3, 10, 1, 2, 11, 0, 0, 12, 2, 2, 12, 0, 2, 13,
    ];
    let farmland = [
        4, 4, 1, 8, 8, 1, 3, 4, 2, 8, 8, 2, 2, 2, 3, 4, 4, 3, 8, 8, 3, 2, 3, 4, 7, 8, 4, 1, 3, 5,
        6, 6, 5, 8, 8, 5, 1, 1, 6, 6, 7, 6, 1, 2, 7, 5, 7, 7, 0, 2, 8, 5, 5, 8, 7, 8, 8, 0, 0, 9,
        5, 8, 9, 0, 0, 10, 4, 8, 10, 0, 0, 11, 3, 4, 11, 6, 7, 11, 3, 6, 12,
    ];
    let older = [
        4, 1, 8, 2, 4, 3, 6, 6, 7, 7, 1, 8, 2, 8, 5, 8, 7, 8, 8, 8, 0, 9, 8, 9, 3, 12,
    ];
    let melons = [3, 2, 7, 4, 1, 5, 2, 5, 6, 5, 7, 6, 4, 10, 3, 11];
    let ponds = [[7, 2], [3, 3], [7, 5], [2, 6], [6, 8], [1, 9], [5, 11]];
    street(c, v, &[[7, 0], [1, 13]]);
    c.runs(&S.path, 0, path.as_chunks().0);
    field(c, farmland.as_chunks().0);
    c.scatter(&S.water, 0, &ponds);
    c.place(&S.grass, 7, 0, 3);
    c.scatter(&wheat(1), 1, older.as_chunks().0);
    c.scatter(&wheat(2), 1, &[[6, 10], [5, 12]]);
    c.scatter(&melon_stem(0), 1, melons.as_chunks().0);
    c.scatter(&melon_stem(1), 1, &[[3, 5], [5, 10]]);
    c.scatter(&melon_stem(2), 1, &[[4, 2], [4, 11]]);
    c.scatter(
        &S.air,
        1,
        &[[7, 2], [3, 3], [7, 3], [6, 4], [7, 5], [5, 11], [2, 13]],
    );
    decorations(c, v, NOTHING, &[[6, 4], [1, 12]]);
}

fn straight_08_of(c: &mut Canvas, v: Village) {
    straight(c, v, 16, &[]);
    field(c, &[[7, 7, 0], [6, 8, 1], [6, 6, 2], [8, 8, 2], [7, 8, 3]]);
    c.scatter(&K.dry_farmland, 0, &[[7, 0], [7, 1], [8, 2]]);
    c.place(&S.water, 7, 0, 2);
    c.place(&wheat(1), 7, 1, 3);
    decorations(c, v, GRASS, &[[6, 4], [5, 6], [5, 13]]);
}

fn straight_09_of(c: &mut Canvas, v: Village) {
    let path = [
        8, 10, 8, 10, 9, 11, 10, 11, 10, 11, 8, 11, 10, 11, 10, 11, 10, 11, 10, 11, 10, 12, 11, 13,
        12, 13, 12, 13, 11, 13, 11, 13,
    ];
    street(c, v, &[[9, 0], [12, 15]]);
    c.scatter(&S.air, 1, &[[10, 9], [11, 14]]);
    c.rows(&S.path, 0, 0, path.as_chunks().0);
    houses(c, v, West, 8, [5, 5]);
    houses(c, v, East, 11, [5, 5]);
    houses(c, v, West, 10, [10, 10]);
    houses(c, v, East, 13, [11, 11]);
    decorations(c, v, GRASS, &[[9, 3], [11, 13]]);
}

fn straight_10_of(c: &mut Canvas, v: Village) {
    street(c, v, &[[4, 0], [1, 10]]);
    c.rows(
        &S.path,
        0,
        0,
        &[[3, 5], [3, 5], [2, 4], [2, 3], [2, 3], [2, 3]],
    );
    c.rows(&S.path, 0, 6, &[[1, 3], [1, 2], [1, 2], [0, 2], [0, 2]]);
    c.each(&S.dirt, &[[4, 0, 0], [1, 0, 10]]);
}

fn straight_11_of(c: &mut Canvas, v: Village) {
    let path = [
        0, 2, 0, 2, 0, 2, 1, 2, 1, 2, 1, 2, 0, 2, 0, 1, 0, 1, 0, 1, 0, 1, 0, 2, 1, 2, 1, 2, 1, 2,
        0, 2, 0, 2,
    ];
    let farmland = [
        9, 9, 0, 13, 13, 0, 4, 5, 1, 8, 11, 1, 13, 15, 1, 3, 4, 2, 6, 12, 2, 14, 16, 2, 3, 7, 3, 9,
        16, 3, 3, 13, 4, 15, 16, 4, 3, 3, 5, 5, 6, 5, 8, 10, 5, 12, 15, 5, 3, 8, 6, 13, 14, 6, 2,
        6, 7, 14, 14, 7, 3, 4, 8, 2, 2, 9, 2, 2, 10,
    ];
    let melons = [
        10, 1, 11, 1, 15, 1, 10, 2, 7, 4, 6, 5, 8, 5, 6, 6, 7, 6, 8, 6, 5, 7, 6, 7,
    ];
    let ponds = [[5, 2], [13, 2], [8, 3], [14, 4], [4, 5], [7, 5], [2, 8]];
    street(c, v, &[[1, 0], [1, 16]]);
    c.rows(&S.path, 0, 0, path.as_chunks().0);
    field(c, farmland.as_chunks().0);
    c.runs(
        &K.dry_farmland,
        0,
        &[[13, 15, 1], [14, 14, 2], [13, 13, 3], [16, 16, 4]],
    );
    c.scatter(&S.water, 0, &ponds);
    c.scatter(
        &wheat(1),
        1,
        &[[6, 3], [10, 3], [6, 4], [13, 5], [4, 6], [4, 7], [2, 10]],
    );
    c.scatter(&melon_stem(0), 1, melons.as_chunks().0);
    c.scatter(&melon_stem(1), 1, &[[15, 2], [5, 6]]);
    c.void([3, 1, 6], [3, 1, 7]);
    houses(c, v, East, 2, [12, 12]);
}

fn turn_01_of(c: &mut Canvas, v: Village) {
    street(c, v, &[[6, 0], [9, 10]]);
    c.solid(&S.air, [14, 1, 0], [18, 1, 10]);
    c.fill(&S.air, 9, 1, [7, 8]);
    c.rows(&S.path, 0, 0, &[[5, 7], [6, 8], [7, 9]]);
    path(c, Z, 9, [3, 10]);
    c.each(&S.dirt, &[[6, 0, 0], [10, 0, 5], [8, 0, 6], [9, 0, 10]]);
    houses(c, v, East, 10, [4, 5]);
    houses(c, v, West, 8, [6, 6]);
}

fn terminator_05_of(c: &mut Canvas, v: Village) {
    let open = [
        3, 4, 4, 4, 15, 4, 13, 5, 3, 6, 4, 6, 2, 7, 5, 7, 6, 7, 4, 8, 5, 8, 4, 9, 5, 9, 5, 11,
    ];
    let streets = if v.zombie {
        v.pool("streets")
    } else {
        EMPTY.to_owned()
    };
    c.jigsaw_layer(1);
    c.scatter(&S.air, 1, open.as_chunks().0);
    c.runs(&S.path, 0, &[[0, 1, 7], [1, 1, 8], [0, 0, 9]]);
    c.fill(&S.dirt, [0, 2], 0, 8);
    c.socket([0, 1, 8], STREET, &streets, NOTHING);
    houses(c, v, East, 2, [8, 8]);
    decorations(c, v, NOTHING, &[[1, 2], [1, 12], [1, 14]]);
}

/// A fence joined to `sides` only, whatever stands next to it.
fn fence(sides: &[Direction]) -> Cell {
    fence_joined("minecraft:acacia_fence", sides)
}

fn slab(kind: &str) -> Cell {
    block(&format!(
        "minecraft:acacia_slab[type={kind},waterlogged=false]"
    ))
}

fn gate(facing: Direction) -> Cell {
    block(&format!(
        "minecraft:acacia_fence_gate[facing={},in_wall=false,open=false,powered=false]",
        facing.name()
    ))
}

fn table(c: &mut Canvas, at: [i32; 3]) {
    c.place(&K.fence, at[0], at[1], at[2]);
    c.place(&K.plate, at[0], at[1] + 1, at[2]);
}

fn turned_stairs(c: &mut Canvas, corners: &[([i32; 3], Direction)]) {
    for ([x, y, z], facing) in corners {
        c.place(&stairs(STAIRS, *facing), *x, *y, *z);
    }
}

/// `block` along the four walls of a round room, a 5x5 square without its
/// corners inside the 7x7 box cornered at `[x, z]`.
fn round_walls(c: &mut Canvas, block: &Cell, [x, z]: [i32; 2], ys: impl Coords + Clone) {
    c.fill(block, x + 2..=x + 4, ys.clone(), [z + 1, z + 5]);
    c.fill(block, [x + 1, x + 5], ys, z + 2..=z + 4);
}

/// `cells[0]` in the middle of the two walls of a round room that run along
/// x, `cells[1]` in the middle of the other two.
fn wall_middles(c: &mut Canvas, [x, y, z]: [i32; 3], cells: [&Cell; 2]) {
    c.fill(cells[0], x + 3, y, [z + 1, z + 5]);
    c.fill(cells[1], [x + 1, x + 5], y, z + 3);
}

/// The room most savanna houses are built round: walls of `post` three high
/// on the layer above `at`, each with a pane in the middle under a `lintel`.
fn round_room(c: &mut Canvas, [x, y, z]: [i32; 3], post: &Cell, lintel: [&Cell; 2]) {
    round_walls(c, post, [x, z], y + 1..=y + 3);
    wall_middles(c, [x, y + 2, z], [&S.pane, &S.pane]);
    wall_middles(c, [x, y + 3, z], lintel);
}

/// The lowest ring of a roof: stairs along the four sides of a rectangle,
/// without its corners, round a ring of planks.
fn eave(c: &mut Canvas, [x, y, z]: [i32; 3], [far_x, far_z]: [i32; 2]) {
    c.fill(&stairs(STAIRS, South), x + 1..=far_x - 1, y, z);
    c.fill(&stairs(STAIRS, North), x + 1..=far_x - 1, y, far_z);
    c.fill(&stairs(STAIRS, East), x, y, z + 1..=far_z - 1);
    c.fill(&stairs(STAIRS, West), far_x, y, z + 1..=far_z - 1);
    c.walls(
        &K.planks,
        &K.planks,
        [x + 1, y, z + 1],
        [far_x - 1, y, far_z - 1],
    );
}

/// One ring of stairs over the eave of a round room, closed by `top`.
fn flat_cap(c: &mut Canvas, [x, y, z]: [i32; 3], top: &Cell) {
    c.hip_roof(STAIRS, None, [x, y, z], [x + 4, z + 4], 1);
    c.solid(top, [x + 1, y, z + 1], [x + 3, y, z + 3]);
}

/// Two rings of stairs closing over a round room, lined with planks; `turned`
/// are the corner stairs that face a quarter away from the ring's.
fn cap(c: &mut Canvas, [x, y, z]: [i32; 3], turned: &[([i32; 3], Direction)]) {
    c.hip_roof(STAIRS, Some(&K.planks), [x, y, z], [x + 4, z + 4], 2);
    c.place(&K.planks, x + 2, y, z + 2);
    turned_stairs(c, turned);
}

fn round_roof(c: &mut Canvas, [x, y, z]: [i32; 3], turned: &[([i32; 3], Direction)]) {
    eave(c, [x, y, z], [x + 6, z + 6]);
    cap(c, [x + 1, y + 1, z + 1], turned);
}

fn footing(c: &mut Canvas, at: [i32; 2]) {
    round_walls(c, &S.dirt, at, 0);
}

fn wood_floor(c: &mut Canvas, grain: &Cell) {
    c.solid(grain, [2, 0, 2], [4, 0, 4]);
    c.each(&K.wood_z, &[[2, 0, 3], [3, 0, 5]]);
}

const HUT_TURNED: [([i32; 3], Direction); 2] = [([1, 5, 1], East), ([5, 5, 1], South)];

/// The terracotta hut of the first three small houses, on open ground with
/// its door to the south.
fn hut(c: &mut Canvas, v: Village, wall: &str) {
    let wall = block(wall);
    let tufts = [
        0, 0, 1, 0, 4, 0, 5, 0, 6, 0, 1, 1, 5, 1, 6, 2, 6, 4, 1, 5, 1, 6,
    ];
    c.solid(&GROUND, [0, 0, 0], [6, 0, 6]);
    wood_floor(c, if v.zombie { &K.wood } else { &K.wood_z });
    round_room(c, [0, 0, 0], &wall, [&wall, &wall]);
    c.door(DOOR, [3, 1, 5], South, Hinge::Right);
    round_roof(c, [0, 4, 0], &HUT_TURNED);
    c.place(&S.wall_torch, 3, 3, 2);
    c.scatter(&S.short_grass, 1, tufts.as_chunks().0);
}

fn small_house_1(c: &mut Canvas, v: Village) {
    hut(c, v, "minecraft:yellow_terracotta");
    table(c, [2, 1, 2]);
    c.bed(ORANGE_BED, [4, 1, 3], North);
    c.scatter(&S.short_grass, 1, &[[3, 0], [0, 3], [4, 6]]);
    c.place(&S.tall_grass, 0, 1, 2);
    if v.zombie {
        c.place(&S.path, 3, 0, 6);
    }
    c.entrance([3, 1, 6], EMPTY, NOTHING);
    villagers(c, v, GRASS, &[[2, 0, 6]]);
}

fn small_house_2(c: &mut Canvas, v: Village) {
    hut(c, v, "minecraft:red_terracotta");
    chest(c, [2, 1, 2], South, "village_savanna_house");
    c.bed(RED_BED, [4, 1, 3], North);
    c.place(&S.wall_torch, 3, 3, 6);
    c.scatter(&S.short_grass, 1, &[[3, 0], [0, 6]]);
    c.place(&S.path, 3, 0, 6);
    c.entrance([4, 1, 6], EMPTY, NOTHING);
    villagers(c, v, GRASS, &[[2, 0, 6]]);
}

fn small_house_3(c: &mut Canvas, v: Village) {
    hut(c, v, "minecraft:orange_terracotta");
    c.place(&block("minecraft:crafting_table"), 4, 1, 2);
    c.scatter(&S.short_grass, 1, &[[0, 6], [4, 6]]);
    if v.zombie {
        c.bed(ORANGE_BED, [2, 1, 3], North);
        c.place(&S.path, 3, 0, 6);
    } else {
        c.bed(RED_BED, [2, 1, 3], North);
        c.fill(&wall_torch(North), [2, 4], 3, 4);
    }
    c.entrance([3, 1, 6], EMPTY, NOTHING);
    villagers(c, v, GRASS, &[[5, 0, 6]]);
}

fn banner(c: &mut Canvas, at: [i32; 3], facing: Direction) {
    let state = format!("minecraft:brown_wall_banner[facing={}]", facing.name());
    c.machine(at, &state, BANNER_DATA);
}

fn small_house_4(c: &mut Canvas, v: Village) {
    let beams = [&K.log_x, &K.log_z];
    let tufts = [
        0, 0, 5, 0, 7, 0, 8, 0, 3, 1, 4, 1, 8, 1, 9, 1, 0, 2, 1, 2, 3, 2, 9, 2, 2, 3, 0, 4, 2, 5,
        0, 6, 9, 6,
    ];
    c.solid(&GROUND, [0, 0, 0], [9, 0, 6]);
    c.solid(&K.planks, [5, 0, 2], [7, 0, 4]);
    c.place(&K.wood_x, 4, 0, 3);
    c.place(&K.wood_z, 6, 0, 5);
    c.place(&S.path, 6, 0, 6);

    round_room(c, [3, 0, 0], &K.log, beams);
    wall_middles(c, [3, 1, 0], beams);
    if v.zombie {
        c.place(&K.log, 6, 1, 1);
    }
    round_walls(c, &K.log, [3, 0], 4);
    wall_middles(c, [3, 4, 0], [&K.fence, &K.fence]);
    round_roof(c, [3, 5, 0], &[([4, 6, 1], East), ([8, 6, 5], West)]);
    c.door(DOOR, [4, 1, 3], West, Hinge::Left);
    c.door(DOOR, [6, 1, 5], South, Hinge::Left);
    c.entrance([5, 1, 6], EMPTY, NOTHING);

    c.posts(&K.fence, &[1], &[1, 5], [1, 3]);
    c.fill(&K.planks, 1, 4, [1, 5]);
    c.fill(&slab("top"), 2..=3, 4, [1, 5]);
    c.solid(&slab("top"), [1, 4, 2], [2, 4, 4]);
    for z in 1..=5 {
        banner(c, [0, 4, z], West);
    }
    for x in 1..=3 {
        banner(c, [x, 4, 0], North);
        banner(c, [x, 4, 6], South);
    }

    c.bed(ORANGE_BED, [7, 1, 3], North);
    chest(c, [7, 1, 4], West, "village_savanna_house");
    c.each(&S.wall_torch, &[[6, 3, 2], [6, 3, 4], [6, 3, 6]]);
    c.scatter(&S.short_grass, 1, tufts.as_chunks().0);
    c.place(&S.tall_grass, 9, 1, 0);
    villagers(c, v, PLANKS, &[[5, 0, 2]]);
}

fn small_house_5(c: &mut Canvas, v: Village) {
    let beams = [&K.log_x, &K.log_z];
    let terracotta = block("minecraft:yellow_terracotta");
    let tufts = [3, 0, 6, 0, 6, 2, 6, 4, 5, 5, 0, 6, 2, 6, 4, 6, 6, 6];
    c.solid(&GROUND, [0, 0, 0], [6, 0, 6]);
    c.solid(&K.planks, [2, 0, 2], [4, 0, 4]);
    c.place(&K.planks, 5, 0, 3);

    round_room(c, [0, 0, 0], &K.log, beams);
    wall_middles(c, [0, 1, 0], beams);
    round_walls(c, &terracotta, [0, 0], 4);
    c.posts(&K.fence, &[1, 5], &[0, 6], [1, 4]);
    c.posts(&K.fence, &[0, 6], &[1, 5], [1, 4]);
    c.patch(
        &slab("bottom"),
        5,
        &Patch::clipped_rectangle([0, 0], [6, 6], 1),
    );
    c.place(&S.air, 3, 5, 3);
    c.walls(&terracotta, &terracotta, [2, 5, 2], [4, 7, 4]);
    c.fill(&K.fence, 3, 6, [2, 4]);
    c.fill(&K.fence, [2, 4], 6, 3);
    let turned = [
        ([2, 9, 2], East),
        ([4, 9, 2], South),
        ([2, 9, 4], North),
        ([4, 9, 4], West),
    ];
    cap(c, [1, 8, 1], &turned);

    c.door(DOOR, [5, 1, 3], East, Hinge::Left);
    c.entrance([6, 1, 3], EMPTY, NOTHING);
    table(c, [2, 1, 2]);
    c.bed(ORANGE_BED, [2, 1, 3], South);
    c.fill(&S.wall_torch, [2, 4, 6], 3, 3);
    c.scatter(&S.short_grass, 1, tufts.as_chunks().0);
    if v.zombie {
        c.each(&K.log, &[[1, 1, 3], [3, 1, 5]]);
        c.place(&S.path, 6, 0, 3);
    }
    villagers(c, v, PLANKS, &[[4, 0, 3]]);
}

fn small_house_6(c: &mut Canvas, v: Village) {
    c.void([0, 0, 0], [6, 0, 6]);
    footing(c, [0, 0]);
    c.solid(&K.planks, [2, 0, 2], [4, 0, 4]);
    c.place(&K.planks, 1, 0, 3);
    c.place(&S.path, 0, 0, 3);
    c.each(&S.grass, &[[0, 0, 4], [3, 0, 6]]);
    c.place(&S.air, 6, 0, 5);
    c.solid(&S.air, [4, 0, 6], [6, 0, 6]);

    round_room(c, [0, 0, 0], &K.log, [&K.log_x, &K.log_z]);
    c.fill(&K.log_z, [2, 4], 2, [1, 5]);
    c.fill(&K.log_x, [1, 5], 2, [2, 4]);
    let turned = [
        ([1, 5, 1], East),
        ([1, 5, 5], North),
        ([5, 5, 5], West),
        ([2, 6, 2], East),
        ([2, 6, 4], North),
    ];
    round_roof(c, [0, 4, 0], &turned);
    c.door(DOOR, [1, 1, 3], West, Hinge::Right);
    c.entrance([0, 1, 3], EMPTY, NOTHING);

    c.place(&stairs(STAIRS, West), 2, 1, 2);
    c.place(&K.wood, 3, 1, 2);
    c.place(&stairs(STAIRS, East), 4, 1, 2);
    c.bed(ORANGE_BED, [3, 1, 4], East);
    c.place(&S.wall_torch, 4, 3, 3);
    c.scatter(&S.short_grass, 1, &[[0, 4], [3, 6]]);
    villagers(c, v, PLANKS, &[[3, 0, 3]]);
    c.spot([6, 0, 3], CATS, GRASS);
}

fn small_house_7(c: &mut Canvas, v: Village) {
    let beams = if v.zombie {
        [&K.log_x, &K.log_z]
    } else {
        [&K.wood, &K.wood]
    };
    let tufts = [
        0, 0, 1, 0, 5, 0, 6, 0, 1, 1, 5, 1, 6, 2, 0, 3, 1, 5, 0, 6, 1, 6, 4, 6,
    ];
    c.solid(&GROUND, [0, 0, 0], [6, 0, 6]);
    wood_floor(c, &K.wood);
    c.place(&S.path, 3, 0, 6);
    round_room(c, [0, 0, 0], &K.log, beams);
    wall_middles(c, [0, 1, 0], beams);
    round_roof(c, [0, 4, 0], &HUT_TURNED);
    if v.zombie {
        c.door(DOOR, [3, 1, 5], South, Hinge::Right);
    } else {
        c.door(DOOR, [3, 1, 5], North, Hinge::Left);
    }
    c.entrance([2, 1, 6], EMPTY, NOTHING);

    c.bed(ORANGE_BED, [2, 1, 3], North);
    chest(c, [2, 1, 4], East, "village_savanna_house");
    c.place(&stairs(STAIRS, North), 4, 1, 2);
    c.place(&K.wood, 4, 1, 3);
    c.place(&S.torch, 4, 2, 3);
    c.place(&stairs(STAIRS, South), 4, 1, 4);
    c.place(&S.wall_torch, 3, 3, 6);
    c.scatter(&S.short_grass, 1, tufts.as_chunks().0);
    villagers(c, v, GRASS, &[[5, 0, 6]]);
}

fn small_house_8(c: &mut Canvas, v: Village) {
    let wall = block("minecraft:yellow_terracotta");
    c.void([0, 0, 0], [5, 0, 6]);
    c.fill(&S.dirt, 2..=3, 0, [1, 5]);
    c.fill(&S.dirt, [1, 4], 0, 2..=4);
    c.fill(&K.log_x, 2..=3, 0, [2, 4]);
    c.place(&K.log_x, 1, 0, 3);
    c.place(&K.log, 2, 0, 3);
    c.place(&S.path, 0, 0, 3);
    c.void([5, 1, 2], [5, 1, 2]);

    for (y, layer) in [(1, &wall), (2, &K.wood), (3, &wall)] {
        c.fill(layer, 2..=3, y, [1, 5]);
        c.fill(layer, [1, 4], y, 2..=4);
    }
    c.place(&S.pane, 4, 2, 3);
    c.door(DOOR, [1, 1, 3], West, Hinge::Right);
    c.entrance([0, 1, 3], EMPTY, NOTHING);

    eave(c, [0, 4, 0], [5, 6]);
    c.hip_roof(STAIRS, Some(&K.planks), [1, 5, 1], [4, 5], 1);
    turned_stairs(
        c,
        &[([4, 5, 1], South), ([4, 5, 5], West), ([1, 5, 5], North)],
    );

    c.bed(ORANGE_BED, [2, 1, 2], East);
    c.fill(&stairs(STAIRS, South), 2..=3, 1, 4);
    c.fill(&S.wall_torch, [0, 2], 3, 3);
    villagers(c, v, LOG, &[[3, 0, 3]]);
    decorations(c, v, GRASS, &[[0, 0]]);
}

/// One of the two rooms of the first medium house, in the 7x7 box that
/// starts at the row `z`, with a door in its west wall.
fn twin_room(c: &mut Canvas, z: i32, turned: &[([i32; 3], Direction)]) {
    footing(c, [0, z]);
    c.solid(&K.planks, [2, 0, z + 2], [4, 0, z + 4]);
    c.place(&K.planks, 1, 0, z + 3);
    c.place(&S.path, 0, 0, z + 3);
    round_room(c, [0, 0, z], &K.log, [&K.log_x, &K.log_z]);
    round_roof(c, [0, 4, z], turned);
    c.door(DOOR, [1, 1, z + 3], West, Hinge::Left);
}

fn medium_house_1(c: &mut Canvas, v: Village) {
    if v.zombie {
        let tufts = [
            0, 9, 0, 10, 0, 13, 0, 14, 1, 14, 3, 14, 4, 14, 5, 14, 6, 11, 6, 13, 6, 14, 7, 11, 7,
            12, 7, 13, 7, 14,
        ];
        c.solid(&S.grass, [0, 0, 0], [7, 0, 14]);
        c.scatter(&S.short_grass, 1, tufts.as_chunks().0);
    } else {
        c.solid(&S.grass, [0, 0, 4], [7, 0, 10]);
        c.each(&S.air, &[[0, 0, 9], [1, 0, 9], [0, 0, 10]]);
    }
    let north = [
        ([1, 5, 1], East),
        ([5, 5, 5], West),
        ([2, 6, 2], East),
        ([4, 6, 2], South),
        ([2, 6, 4], North),
    ];
    twin_room(c, 0, &north);
    twin_room(
        c,
        8,
        &[([1, 5, 9], East), ([5, 5, 13], West), ([2, 6, 10], East)],
    );
    c.fill(&K.planks, 3, 0, [5, 9]);
    c.door(DOOR, [3, 1, 5], South, Hinge::Right);
    c.door(DOOR, [3, 1, 9], North, Hinge::Left);
    c.place(&S.dirt, 0, 0, 7);
    c.entrance([0, 1, 7], EMPTY, NOTHING);

    c.solid(&S.farmland, [5, 0, 5], [6, 0, 9]);
    c.solid(&wheat(7), [5, 1, 5], [6, 1, 9]);
    c.place(&S.water, 6, 0, 7);
    c.place(&S.air, 6, 1, 7);
    c.fill(&K.fence, 6..=7, 1, [4, 10]);
    c.solid(&K.fence, [7, 1, 5], [7, 1, 9]);
    c.solid(&K.fence, [2, 1, 6], [2, 1, 8]);
    c.place(&S.short_grass, 1, 1, 7);

    chest(c, [2, 1, 2], South, "village_savanna_house");
    for z in [2, 12] {
        c.bed(ORANGE_BED, [3, 1, z], East);
    }
    c.place(&block("minecraft:crafting_table"), 4, 1, 4);
    c.place(&K.wood, 4, 1, 10);
    c.place(&S.torch, 4, 2, 10);
    c.place(&S.wall_torch, 4, 3, 3);
    if v.lit() {
        c.fill(&S.torch, 7, 2, [4, 10]);
        c.fill(&S.wall_torch, 0, 3, [3, 11]);
        c.fill(&S.wall_torch, 3, 3, [6, 8]);
    }
    villagers(c, v, PLANKS, &[[3, 0, 3], [3, 0, 11]]);
    decorations(c, v, GRASS, &[[0, 8]]);
}

fn medium_house_2(c: &mut Canvas, v: Village) {
    let beams = [&K.wood_x, &K.wood_z];
    c.void([0, 0, 0], [9, 7, 10]);
    c.each(
        &S.air,
        &[[9, 1, 0], [3, 1, 3], [7, 1, 8], [9, 1, 9], [9, 1, 10]],
    );

    for [x, z] in [[0, 0], [3, 4]] {
        footing(c, [x, z]);
        c.solid(&K.planks, [x + 2, 0, z + 2], [x + 4, 0, z + 4]);
        round_room(c, [x, 0, z], &K.wood, beams);
        wall_middles(c, [x, 1, z], beams);
        eave(c, [x, 4, z], [x + 6, z + 6]);
        flat_cap(c, [x + 1, 5, z + 1], &slab("top"));
    }
    // Where the two eaves cross, the first room's ring of planks stays whole
    // and the stairs inside either room are left out.
    c.void([4, 4, 4], [4, 4, 4]);
    c.void([5, 4, 6], [5, 4, 6]);
    c.each(&K.planks, &[[5, 4, 4], [3, 4, 5]]);
    c.place(&stairs(STAIRS, North), 3, 4, 6);
    turned_stairs(
        c,
        &[([1, 5, 1], East), ([1, 5, 5], North), ([4, 5, 5], North)],
    );

    c.each(&K.wood, &[[1, 0, 3], [6, 0, 9]]);
    c.open_door("minecraft:acacia_door", [1, 1, 3], South, Hinge::Left);
    c.door(DOOR, [6, 1, 9], South, Hinge::Right);
    c.place(&S.dirt, 0, 0, 5);
    c.entrance([0, 1, 5], EMPTY, NOTHING);
    c.fill(&S.path, 0, 0, [3, 4, 6]);
    c.runs(
        &S.path,
        0,
        &[[1, 1, 6], [1, 1, 7], [2, 2, 8], [2, 3, 9], [4, 6, 10]],
    );
    field(c, &[[2, 2, 6], [2, 3, 7], [3, 3, 8]]);
    c.place(&wheat(1), 3, 1, 7);
    c.place(&S.water, 3, 0, 6);

    c.place(&block("minecraft:crafting_table"), 2, 1, 2);
    c.bed(ORANGE_BED, [3, 1, 2], East);
    c.fill(&stairs(STAIRS, South), 2..=3, 1, 4);
    c.place(&K.planks, 4, 1, 4);
    c.place(&block("minecraft:potted_dandelion"), 4, 2, 4);
    c.bed(ORANGE_BED, [5, 1, 7], North);
    c.place(&stairs(STAIRS, West), 5, 1, 8);
    chest(c, [7, 1, 6], West, "village_savanna_house");
    c.each(&S.wall_torch, &[[4, 3, 3], [6, 3, 6]]);
    villagers(c, v, PLANKS, &[[3, 0, 3], [7, 0, 8]]);
    decorations(c, v, GRASS, &[[7, 2]]);
}

fn savanna_animal_pen_1(c: &mut Canvas) {
    c.solid(&GROUND, [1, 0, 2], [7, 0, 7]);
    c.walls(&K.log, &K.log, [0, 0, 1], [8, 0, 8]);
    c.solid(&S.dirt, [1, 0, 2], [1, 0, 7]);
    c.solid(&S.dirt, [2, 0, 2], [5, 0, 2]);
    c.each(&S.dirt, &[[5, 0, 3], [3, 0, 4], [7, 0, 5]]);
    c.fill(&S.grass, [3, 5], 0, 5);
    c.solid(&S.grass, [3, 0, 6], [5, 0, 6]);

    c.fence_ring(&K.fence, &gate(South), [0, 1, 1], [8, 8], &[[4, 1]]);
    c.posts(&K.fence, &[0, 4, 8], &[4], [1, 3]);
    c.solid(&K.planks, [2, 1, 5], [6, 1, 6]);
    c.solid(&S.water, [3, 1, 6], [5, 1, 6]);
    c.solid(&K.log, [2, 1, 7], [6, 1, 7]);
    let roof = [
        (2, 8, "bottom"),
        (2, 7, "top"),
        (3, 6, "bottom"),
        (3, 5, "top"),
        (4, 4, "bottom"),
    ];
    for (y, z, kind) in roof {
        c.solid(&slab(kind), [0, y, z], [8, y, z]);
    }
    c.solid(&slab("double"), [2, 2, 7], [6, 2, 7]);
    c.entrance([4, 0, 0], EMPTY, STEP_SOUTH);
    c.spot([3, 0, 2], ANIMALS, GRASS);
}

fn animal_pen_2(c: &mut Canvas, v: Village) {
    let mound = [
        2, 6, 0, 2, 9, 1, 0, 9, 2, 1, 10, 3, 0, 12, 4, 0, 12, 5, 0, 12, 6, 0, 12, 7, 2, 10, 8, 5,
        10, 9, 1, 1, 10, 5, 10, 10, 5, 10, 11,
    ];
    let fence = [
        2, 0, 3, 0, 5, 0, 6, 0, 2, 1, 6, 1, 7, 1, 8, 1, 9, 1, 1, 2, 2, 2, 9, 2, 1, 3, 9, 3, 10, 3,
        0, 4, 1, 4, 10, 4, 11, 4, 12, 4, 0, 5, 12, 5, 0, 6, 12, 6, 0, 7, 1, 7, 2, 7, 10, 7, 11, 7,
        12, 7, 2, 8, 3, 8, 4, 8, 5, 8, 10, 8, 5, 9, 10, 9, 5, 10, 10, 10, 5, 11, 6, 11, 7, 11, 8,
        11, 9, 11, 10, 11,
    ];
    let tufts = [
        3, 1, 4, 1, 3, 2, 4, 2, 5, 2, 8, 2, 4, 3, 5, 3, 7, 3, 8, 3, 5, 4, 8, 4, 1, 5, 2, 5, 3, 5,
        4, 5, 8, 5, 9, 5, 11, 5, 1, 6, 4, 6, 5, 6, 6, 6, 9, 6, 11, 6, 4, 7, 6, 8, 7, 8, 9, 8, 9, 9,
        7, 10,
    ];
    let tall = [3, 3, 6, 4, 10, 6, 6, 9, 8, 9, 6, 10, 8, 10];
    c.void([0, 0, 0], [12, 1, 11]);
    c.runs(&S.dirt, 0, mound.as_chunks().0);
    c.runs(&GROUND, 1, mound.as_chunks().0);
    c.fill(&S.air, 0, 1, [0, 1, 9]);
    c.void([1, 2, 0], [1, 2, 1]);
    c.scatter(&S.water, 1, &[[7, 5], [3, 6], [8, 6]]);

    c.scatter(&K.fence, 2, fence.as_chunks().0);
    c.posts(&K.fence, &[3, 5], &[0], [2, 4]);
    c.place(&K.fence, 4, 4, 0);
    c.fill(&S.torch, [3, 5], 5, 0);
    c.each(&S.torch, &[[0, 3, 4], [12, 3, 4], [5, 3, 8]]);
    c.entrance([4, 2, 0], EMPTY, "minecraft:acacia_fence_gate");
    c.scatter(&S.short_grass, 2, tufts.as_chunks().0);
    c.scatter(&S.tall_grass, 2, tall.as_chunks().0);
    spots(c, CATS, GRASS, &[[0, 1, 2], [1, 1, 10]]);
    spots(c, ANIMALS, GRASS, &[[4, 1, 4], [8, 1, 8]]);
    c.spot([7, 1, 6], &v.pool("decor"), GRASS);
}

fn animal_pen_3(c: &mut Canvas, v: Village) {
    let tufts = [
        4, 1, 6, 1, 1, 2, 2, 2, 3, 2, 4, 2, 1, 3, 3, 3, 6, 3, 2, 4, 5, 4, 2, 5, 3, 6, 5, 6, 6, 6,
        6, 7,
    ];
    c.solid(&GROUND, [0, 0, 0], [7, 0, 8]);
    c.place(&S.grass, 4, 0, 8);
    c.fence_ring(&K.fence, &gate(North), [0, 1, 0], [7, 8], &[[2, 8]]);
    c.posts(&K.fence, &[2, 5], &[0, 3], [1, 3]);
    c.fill(&S.torch, [3, 4], 2, 0);
    c.walls(&slab("bottom"), &slab("bottom"), [2, 4, 0], [5, 4, 3]);
    c.solid(&slab("double"), [3, 4, 1], [4, 4, 2]);
    c.scatter(&S.short_grass, 1, tufts.as_chunks().0);
    c.entrance(
        [4, 1, 8],
        EMPTY,
        "minecraft:acacia_fence[east=true,west=true]",
    );
    c.spot([4, 0, 5], ANIMALS, GRASS);
    decorations(c, v, GRASS, &[[2, 6]]);
}

fn savanna_large_farm_1(c: &mut Canvas) {
    let beds = [1..=3, 5..=7];
    let composter = block("minecraft:composter[level=0]");
    c.void([0, 3, 0], [8, 5, 8]);
    c.solid(&GROUND, [0, 0, 0], [8, 0, 8]);
    c.walls(&K.log, &composter, [0, 1, 0], [8, 1, 8]);
    c.place(&S.air, 8, 1, 0);
    c.place(&S.short_grass, 0, 1, 8);
    c.fill(&K.planks, 4, 1, 1..=7);
    c.fill(&K.planks, 1..=7, 1, 4);
    c.fill(&S.farmland, beds.clone(), 1, beds.clone());
    c.fill(&wheat(7), beds.clone(), 2, beds);
    c.fill(&S.grass, [3, 5], 0, [3, 5]);
    c.fill(&S.water, [3, 5], 1, [3, 5]);
    c.fill(&S.air, [3, 5], 2, [3, 5]);
    c.place(&stairs(STAIRS, South), 4, 1, 0);
    c.place(&stairs(STAIRS, East), 0, 1, 4);
    c.place(&stairs(STAIRS, West), 8, 1, 4);
    c.place(&stairs(STAIRS, North), 4, 1, 8);
    c.entrance([3, 1, 8], EMPTY, LOG);
}

fn large_farm_2(c: &mut Canvas, v: Village) {
    let plot = [
        1, 1, 0, 4, 6, 0, 0, 8, 1, 1, 9, 2, 0, 9, 3, 0, 9, 4, 1, 9, 5, 1, 9, 6, 1, 9, 7,
    ];
    let grass = [
        6, 0, 0, 1, 2, 1, 3, 1, 7, 1, 3, 2, 0, 3, 6, 4, 8, 4, 1, 5, 4, 6, 4, 7, 7, 7, 9, 7,
    ];
    let path = [
        8, 1, 2, 2, 9, 2, 3, 3, 0, 4, 1, 4, 4, 5, 8, 5, 9, 5, 5, 6, 9, 6, 6, 7,
    ];
    let ripe = [
        1, 0, 4, 0, 1, 1, 4, 1, 6, 1, 4, 2, 5, 2, 6, 2, 8, 2, 4, 3, 2, 4, 3, 4, 5, 4, 9, 4, 3, 5,
        6, 5, 7, 5, 2, 6, 3, 6, 6, 6, 7, 6,
    ];
    let ripening = [7, 2, 5, 3, 9, 3, 4, 4, 2, 5, 1, 7];
    let tufts = [0, 1, 2, 1, 7, 1, 0, 3, 6, 4, 4, 7, 9, 7];
    let tall = [6, 0, 3, 1, 8, 4, 1, 5, 4, 6, 7, 7];
    c.void([0, 0, 0], [9, 1, 7]);
    c.runs(&GROUND, 0, plot.as_chunks().0);
    c.scatter(&S.dirt, 0, &[[4, 3], [3, 4]]);
    c.place(&S.grass, 8, 0, 7);
    c.place(&S.air, 0, 0, 5);
    c.runs(&S.farmland, 1, plot.as_chunks().0);
    c.scatter(&S.grass, 1, grass.as_chunks().0);
    c.scatter(&S.path, 1, path.as_chunks().0);
    c.scatter(
        &S.water,
        1,
        &[[5, 1], [1, 3], [8, 3], [5, 5], [2, 7], [8, 7]],
    );
    c.place(&S.dirt, 5, 1, 0);
    c.scatter(&wheat(7), 2, ripe.as_chunks().0);
    c.scatter(&wheat(6), 2, ripening.as_chunks().0);
    c.scatter(&wheat(5), 2, &[[7, 3], [7, 4], [8, 6]]);
    c.scatter(&wheat(4), 2, &[[1, 6], [3, 7]]);
    c.place(&wheat(2), 5, 2, 7);
    c.scatter(&wheat(0), 2, &[[1, 2], [2, 3]]);
    c.place(&block("minecraft:composter[level=0]"), 3, 2, 2);
    c.scatter(&S.short_grass, 2, tufts.as_chunks().0);
    c.scatter(&S.tall_grass, 2, tall.as_chunks().0);
    c.entrance([5, 2, 0], EMPTY, NOTHING);
    c.spot([6, 1, 3], &v.pool("decor"), GRASS);
}

fn savanna_small_farm(c: &mut Canvas) {
    let plot = [
        0, 5, 0, 1, 5, 1, 1, 5, 2, 0, 5, 3, 0, 5, 4, 0, 5, 5, 0, 4, 6, 0, 5, 7, 0, 4, 8,
    ];
    let grass = [0, 0, 1, 1, 5, 2, 2, 4, 4, 5, 5, 5, 0, 6, 4, 7, 5, 7, 1, 8];
    let ripe = [
        5, 0, 3, 1, 5, 1, 1, 2, 3, 2, 0, 3, 1, 3, 2, 3, 4, 3, 5, 3, 0, 4, 1, 4, 4, 4, 5, 4, 3, 5,
        1, 6, 3, 6, 4, 6, 1, 7,
    ];
    let sprouts = [2, 0, 4, 0, 4, 2, 3, 3, 2, 6, 2, 7, 3, 7, 2, 8, 3, 8];
    let tall = [0, 0, 1, 1, 5, 2, 2, 4, 5, 5, 0, 6, 4, 7];
    let ponds = [[4, 1], [2, 2], [2, 5]];
    c.void([0, 0, 0], [5, 1, 8]);
    c.void([0, 4, 0], [5, 6, 8]);
    c.runs(&GROUND, 0, plot.as_chunks().0);
    c.scatter(&S.grass, 0, &ponds);
    c.runs(&S.farmland, 1, plot.as_chunks().0);
    c.place(&K.dry_farmland, 4, 1, 0);
    c.scatter(&S.grass, 1, grass.as_chunks().0);
    c.scatter(&S.water, 1, &ponds);
    c.scatter(&S.dirt, 1, &[[1, 0], [4, 8]]);
    c.scatter(&wheat(7), 2, ripe.as_chunks().0);
    c.scatter(&wheat(6), 2, &[[0, 5], [0, 8]]);
    c.scatter(&wheat(5), 2, &[[1, 5], [0, 7]]);
    c.scatter(&wheat(1), 2, &[[3, 0], [2, 1], [3, 4]]);
    c.scatter(&wheat(0), 2, sprouts.as_chunks().0);
    c.place(&block("minecraft:composter[level=0]"), 4, 2, 5);
    c.place(&block("minecraft:melon"), 4, 2, 8);
    c.scatter(&S.short_grass, 2, &[[5, 7], [1, 8]]);
    c.scatter(&S.tall_grass, 2, tall.as_chunks().0);
    c.entrance([1, 2, 0], EMPTY, NOTHING);
}

fn savanna_armorer_1(c: &mut Canvas) {
    let terracotta = block("minecraft:orange_terracotta");
    c.solid(&GROUND, [0, 0, 0], [6, 0, 6]);
    c.solid(&K.planks, [2, 0, 2], [2, 0, 4]);
    c.solid(&terracotta, [3, 0, 2], [3, 0, 4]);
    c.place(&K.log_x, 1, 0, 3);
    c.place(&S.path, 0, 0, 3);
    round_walls(c, &K.log, [0, 0], 1..=3);
    c.place(&K.log_z, 1, 3, 3);
    eave(c, [0, 4, 0], [6, 6]);
    flat_cap(c, [1, 5, 1], &K.planks);
    c.place(&stairs(STAIRS, North), 1, 5, 5);
    c.door(DOOR, [1, 1, 3], West, Hinge::Right);
    c.entrance([0, 1, 3], EMPTY, NOTHING);

    c.fill(&terracotta, 4, 1..=2, [2, 4]);
    c.solid(&terracotta, [4, 3, 3], [4, 5, 3]);
    c.furnace([4, 1, 3], "blast_furnace", West);
    c.place(
        &block("minecraft:orange_glazed_terracotta[facing=west]"),
        4,
        2,
        3,
    );
    c.place(&S.wall_torch, 2, 3, 3);
    c.solid(&S.short_grass, [0, 1, 0], [6, 1, 0]);
    c.scatter(&S.short_grass, 1, &[[0, 1], [1, 1], [6, 1], [6, 2]]);
}

/// The long room of the weaponsmith and the tannery: walls of `wall` round a
/// 2x5 floor, two windows to the west, the door to the east and a roof whose
/// corner stairs are `turned`.
fn workshop(c: &mut Canvas, wall: &Cell, turned: &[([i32; 3], Direction)]) {
    c.fill(wall, 2..=3, 1..=3, [1, 7]);
    c.fill(wall, [1, 4], 1..=3, 2..=6);
    c.fill(&S.pane, 1, 2, [3, 5]);
    c.door(DOOR, [4, 1, 4], East, Hinge::Right);
    eave(c, [0, 4, 0], [5, 8]);
    c.hip_roof(STAIRS, Some(&K.planks), [1, 5, 1], [4, 7], 1);
    turned_stairs(c, turned);
}

fn savanna_weaponsmith_1(c: &mut Canvas) {
    let stripped = log("minecraft:stripped_acacia_log", Y);
    let stripped_x = log("minecraft:stripped_acacia_log", X);
    c.solid(&GROUND, [0, 0, 0], [7, 0, 8]);
    c.fill(&stripped, 2..=3, 0, [2, 4, 6]);
    c.fill(&stripped_x, 2..=3, 0, [3, 5]);
    c.place(&stripped, 4, 0, 4);
    c.solid(&S.path, [5, 0, 4], [7, 0, 4]);
    c.each(&S.air, &[[0, 0, 7], [1, 0, 8], [3, 0, 8], [4, 0, 8]]);
    c.fill(&S.short_grass, [0, 2], 0, 8);

    let turned = [
        ([1, 5, 1], East),
        ([4, 5, 1], South),
        ([1, 5, 7], North),
        ([4, 5, 7], West),
    ];
    workshop(c, &K.log, &turned);
    c.fill(&K.log_z, 1, 3, [3, 5]);
    c.place(&K.log_z, 4, 3, 4);
    c.entrance([7, 1, 4], EMPTY, NOTHING);
    c.fill(&K.fence, 5..=7, 1, [2, 6]);
    c.place(&fence(&[North]), 7, 1, 3);
    c.place(&fence(&[South]), 7, 1, 5);

    c.place(&K.fence, 2, 1, 2);
    c.place(&block("minecraft:white_carpet"), 2, 2, 2);
    c.place(&stairs(STAIRS, North), 3, 1, 2);
    c.place(
        &block("minecraft:grindstone[face=floor,facing=south]"),
        2,
        1,
        6,
    );
    c.fill(&S.wall_torch, 2, 3, [3, 5]);
    for z in [3, 5] {
        banner(c, [5, 3, z], East);
    }
    let tufts = [5, 0, 7, 0, 4, 1, 5, 1, 5, 3, 5, 5, 6, 5, 5, 7];
    c.scatter(&S.short_grass, 1, tufts.as_chunks().0);
}

fn savanna_tannery_1(c: &mut Canvas) {
    let wall = block("minecraft:yellow_terracotta");
    let cauldron = block("minecraft:water_cauldron[level=3]");
    let tufts = [
        1, 0, 2, 0, 3, 0, 5, 0, 6, 0, 0, 1, 6, 1, 0, 2, 0, 3, 7, 3, 0, 4, 0, 5, 0, 6, 6, 6, 4, 7,
        6, 7, 2, 8, 4, 8, 6, 8,
    ];
    c.solid(&GROUND, [0, 0, 0], [7, 0, 8]);
    c.fill(&K.log_z, 2..=3, 0, [2, 6]);
    c.solid(&K.log, [2, 0, 3], [3, 0, 5]);
    c.place(&K.log_x, 4, 0, 4);
    c.solid(&S.path, [7, 0, 0], [7, 0, 8]);
    c.place(&S.grass, 7, 0, 3);
    c.place(&S.air, 7, 0, 4);
    c.solid(&S.path, [5, 0, 4], [6, 0, 4]);

    workshop(
        c,
        &wall,
        &[([4, 5, 1], South), ([1, 5, 7], North), ([4, 5, 7], West)],
    );
    c.socket_facing([6, 1, 4], "east_up", ENTRANCE, EMPTY, NOTHING);
    c.fill(&fence(&[]), 6, 1..=2, [3, 5]);
    for z in [3, 5] {
        c.place(&slab("top"), 5, 3, z);
        c.place(&K.planks, 6, 3, z);
        banner(c, [7, 3, z], East);
    }
    c.place(&slab("top"), 6, 3, 4);

    c.solid(&cauldron, [2, 1, 2], [3, 1, 2]);
    chest(c, [2, 1, 6], North, "village_tannery");
    c.place(&block("minecraft:smooth_stone"), 3, 1, 6);
    c.fill(&S.wall_torch, [2, 3, 5], 3, 4);
    c.scatter(&S.short_grass, 1, tufts.as_chunks().0);
}

fn savanna_temple_2(c: &mut Canvas) {
    let red = block("minecraft:red_terracotta");
    let yellow = block("minecraft:yellow_terracotta");
    c.fill(&S.dirt, 2..=4, 0, [1, 7]);
    c.fill(&S.dirt, [1, 5], 0, 2..=6);
    c.solid(&K.wood, [2, 0, 2], [4, 0, 6]);
    c.fill(&K.wood_z, [2, 4], 0, 3);
    c.solid(&red, [2, 0, 4], [4, 0, 4]);
    c.place(&K.wood_x, 1, 0, 4);
    c.place(&S.dirt, 0, 0, 4);

    c.fill(&K.wood, 2..=4, 1..=3, [1, 7]);
    c.fill(&K.wood, [1, 5], 1..=3, 2..=6);
    c.fill(&K.wood_x, 3, [1, 3], [1, 7]);
    c.fill(&S.pane, 3, 2, [1, 7]);
    c.place(&K.log, 2, 1, 1);
    c.fill(&K.wood_z, 5, [1, 3], [3, 5]);
    c.fill(&S.pane, 5, 2, [3, 5]);
    c.place(&K.wood_z, 1, 3, 4);
    c.door(DOOR, [1, 1, 4], West, Hinge::Right);
    c.entrance([0, 1, 4], EMPTY, NOTHING);

    let aisles = [1, 2, 6, 7];
    c.fill(&stairs(STAIRS, South), 1..=5, 4, 0);
    c.fill(&stairs(STAIRS, North), 1..=5, 4, 8);
    c.fill(&stairs(STAIRS, East), 0, 4, aisles);
    c.fill(&stairs(STAIRS, West), 6, 4, aisles);
    c.fill(&K.planks, 1..=5, 4, aisles);
    for z in [3, 5] {
        c.fill(&red, [1, 3, 5], 4, z);
        c.fill(&yellow, [2, 4], 4, z);
    }
    c.fill(&yellow, [1, 5], 4, 4);
    c.fill(&S.wall_torch, [0, 6], 4, 4);
    c.walls(&K.wood, &K.wood, [1, 5, 3], [5, 5, 5]);
    c.fill(&stairs(STAIRS, South), 1..=5, 6, 2);
    c.fill(&stairs(STAIRS, North), 1..=5, 6, 6);
    c.fill(&stairs(STAIRS, East), 0, 6, 3..=5);
    c.fill(&stairs(STAIRS, West), 6, 6, 3..=5);
    c.solid(&K.planks, [1, 6, 3], [5, 6, 5]);

    c.brewing_stand([3, 1, 2]);
    c.solid(&stairs(STAIRS, South), [2, 1, 6], [4, 1, 6]);
    c.fill(&S.wall_torch, 2, 2, [3, 5]);
    c.place(&S.wall_torch, 4, 2, 4);
}

fn savanna_mason_1(c: &mut Canvas) {
    let clay = block("minecraft:clay");
    let glazed = |facing: Direction| {
        block(&format!(
            "minecraft:yellow_glazed_terracotta[facing={}]",
            facing.name()
        ))
    };
    let tufts = [
        0, 0, 1, 0, 2, 0, 0, 1, 2, 1, 0, 2, 0, 3, 0, 6, 0, 7, 1, 7, 0, 8, 2, 8, 1, 9,
    ];
    c.solid(&GROUND, [0, 0, 0], [7, 0, 9]);
    c.solid(&K.planks, [2, 0, 3], [5, 0, 6]);
    c.fill(&K.planks, 3..=5, 0, [2, 7]);
    c.fill(&K.log_x, 1, 0, [4, 5]);

    for (y, corner) in [(1, &K.log), (2, &K.log), (3, &glazed(East))] {
        c.fill(&K.log, 3..=5, y, [1, 8]);
        c.solid(&K.log, [6, y, 2], [6, y, 7]);
        c.each(corner, &[[2, y, 2], [1, y, 3], [1, y, 6], [2, y, 7]]);
    }
    c.each(&glazed(South), &[[2, 3, 2], [1, 3, 3], [1, 3, 5]]);
    c.place(&glazed(East), 1, 3, 4);
    c.fill(&S.pane, 4, 2, [1, 8]);
    c.fill(&K.log_x, 4, 3, [1, 8]);
    c.fill(&S.pane, 6, 2, [3, 6]);
    c.fill(&K.log_z, 6, 3, [3, 6]);
    c.door(DOOR, [1, 1, 4], West, Hinge::Right);
    c.door(DOOR, [1, 1, 5], West, Hinge::Left);
    c.entrance([0, 1, 5], EMPTY, NOTHING);

    let (east, west) = (stairs(STAIRS, East), stairs(STAIRS, West));
    c.fill(&stairs(STAIRS, South), 3..=5, 4, 0);
    c.fill(&stairs(STAIRS, North), 3..=5, 4, 9);
    c.fill(&K.planks, 3..=5, 4, [1, 8]);
    c.fill(&K.planks, [2, 6], 4, [2, 7]);
    c.fill(&K.planks, [1, 6], 4, 3..=6);
    c.fill(&east, 2, 4, [1, 8]);
    c.fill(&east, 1, 4, [2, 7]);
    c.fill(&east, 0, 4, 3..=6);
    c.fill(&west, 6, 4, [1, 8]);
    c.fill(&west, 7, 4, 2..=7);
    c.place(&stairs(STAIRS, South), 4, 5, 1);
    c.place(&stairs(STAIRS, North), 4, 5, 8);
    c.fill(&K.planks, 3..=5, 5, [2, 7]);
    c.solid(&K.planks, [2, 5, 3], [5, 5, 6]);
    c.fill(&east, 3, 5, [1, 8]);
    c.fill(&east, 2, 5, [2, 7]);
    c.fill(&east, 1, 5, 3..=6);
    c.fill(&west, 5, 5, [1, 8]);
    c.fill(&west, 6, 5, 2..=7);

    c.place(&block("minecraft:stonecutter[facing=north]"), 4, 1, 2);
    for z in [3, 6] {
        table(c, [2, 1, z]);
    }
    c.each(&clay, &[[5, 1, 4], [5, 1, 6], [4, 1, 7]]);
    c.solid(&clay, [5, 1, 7], [5, 3, 7]);
    chest(c, [5, 1, 5], West, "village_mason");
    c.fill(&wall_torch(West), 5, 3, [3, 6]);
    c.scatter(&S.short_grass, 1, tufts.as_chunks().0);
}

fn savanna_temple_1(c: &mut Canvas) {
    let orange = block("minecraft:orange_terracotta");
    let yellow = block("minecraft:yellow_terracotta");
    let orange_pane = settled("minecraft:orange_stained_glass_pane[waterlogged=false]");
    let yellow_pane = settled("minecraft:yellow_stained_glass_pane[waterlogged=false]");
    let carpet = block("minecraft:red_carpet");
    let tufts = [
        1, 0, 3, 0, 5, 0, 6, 0, 7, 0, 8, 0, 9, 0, 11, 0, 0, 1, 2, 1, 3, 1, 5, 1, 7, 1, 12, 1, 2, 2,
        4, 2, 12, 2, 0, 3, 2, 3, 6, 3, 12, 4, 0, 5, 6, 5, 12, 5, 3, 6, 12, 6, 1, 7, 2, 7, 3, 7, 5,
        7, 7, 7, 11, 7, 12, 7, 1, 8, 3, 8, 6, 8, 8, 8, 12, 8,
    ];
    c.solid(&GROUND, [0, 0, 0], [12, 0, 8]);
    c.solid(&K.planks, [8, 0, 2], [10, 0, 6]);
    c.place(&K.log_x, 7, 0, 4);
    c.walls(&S.path, &S.path, [3, 0, 3], [5, 0, 5]);
    c.solid(&S.path, [0, 0, 4], [2, 0, 4]);
    c.place(&S.path, 6, 0, 4);

    c.fill(&K.log, 8..=10, 1..=4, [1, 7]);
    c.fill(&K.log, [7, 11], 1..=4, 2..=6);
    c.fill(&orange_pane, 9, 2, [1, 7]);
    c.fill(&yellow_pane, 9, 3, [1, 7]);
    c.fill(&K.log_x, 9, 4, [1, 7]);
    c.fill(&orange_pane, 11, 2, [3, 5]);
    c.fill(&yellow_pane, 11, 3, [3, 5]);
    c.fill(&K.log_z, 11, 4, [3, 5]);
    c.solid(&K.log_x, [7, 4, 2], [7, 4, 6]);
    c.door(DOOR, [7, 1, 4], West, Hinge::Left);
    c.entrance([0, 1, 4], EMPTY, NOTHING);
    eave(c, [6, 5, 0], [12, 8]);
    c.hip_roof(STAIRS, None, [7, 6, 1], [11, 7], 1);
    c.solid(&K.planks, [8, 6, 2], [10, 6, 6]);
    turned_stairs(c, &[([11, 6, 1], South), ([7, 6, 7], North)]);

    for [x, z] in [[4, 0], [1, 2], [1, 6], [4, 8]] {
        c.solid(&orange, [x, 1, z], [x, 2, z]);
        c.place(&yellow, x, 3, z);
    }
    for z in [2, 6] {
        banner(c, [0, 3, z], West);
    }

    c.solid(&K.wood, [8, 1, 2], [10, 1, 2]);
    c.solid(&carpet, [8, 2, 2], [9, 2, 2]);
    c.brewing_stand([10, 2, 2]);
    c.fill(&stairs(STAIRS, South), [8, 10], 1, 6);
    c.place(&K.wood, 9, 1, 6);
    c.place(&carpet, 9, 2, 6);
    c.place(&S.wall_torch, 8, 3, 4);
    c.fill(&S.wall_torch, 10, 4, [3, 5]);
    c.scatter(&S.short_grass, 1, tufts.as_chunks().0);
}

fn savanna_library_1(c: &mut Canvas) {
    let wall = block("minecraft:orange_terracotta");
    let sapling = block("minecraft:acacia_sapling[stage=1]");
    let poppy = block("minecraft:poppy");
    let white = block("minecraft:white_carpet");
    let orange = block("minecraft:orange_carpet");
    let tufts = [
        1, 0, 8, 0, 0, 1, 9, 1, 0, 3, 4, 3, 5, 3, 8, 3, 9, 3, 0, 4, 2, 4, 4, 4, 6, 4, 7, 4, 9, 4,
        4, 5, 0, 7, 1, 7, 2, 7,
    ];
    c.solid(&GROUND, [0, 0, 0], [9, 0, 7]);
    c.place(&S.short_grass, 9, 0, 6);
    c.place(&S.air, 9, 0, 7);

    c.fill(&K.log, [2, 3, 6, 7], 1, [0, 3]);
    c.fill(&K.log, [1, 8], 1, [1, 2, 4]);
    c.fill(&K.log_z, [1, 8], 1, 5);
    c.fill(&K.log_z, [2, 4, 6], 1, 6);
    c.fill(&K.log, [3, 5, 7], 1, 6);
    c.fill(&S.grass, [2, 3, 6, 7], 1, [1, 2]);
    c.solid(&K.planks, [4, 1, 1], [5, 1, 2]);
    c.place(&stairs(STAIRS, South), 4, 1, 0);
    c.entrance([5, 1, 0], EMPTY, STEP_SOUTH);
    c.fill(&S.torch, [3, 6], 2, 0);
    c.fill(&sapling, [2, 7], 2, 1);
    c.each(&poppy, &[[3, 2, 1], [7, 2, 2]]);
    c.each(&S.short_grass, &[[6, 2, 1], [2, 2, 2]]);
    c.fill(&S.tall_grass, [3, 6], 2, 2);
    c.solid(&stairs(STAIRS, South), [4, 2, 1], [5, 2, 1]);
    c.solid(&K.log, [4, 2, 2], [5, 2, 2]);

    c.solid(&K.log, [2, 2, 3], [7, 2, 3]);
    c.solid(&K.planks, [2, 2, 4], [7, 2, 5]);
    c.fill(&K.log, [1, 8], 2, 4);
    c.fill(&K.log_z, [1, 8], 2, 5);
    c.fill(&K.log, [2, 4, 6], 2, 6);
    c.fill(&K.log_z, [3, 5, 7], 2, 6);

    c.fill(&wall, [2, 7], 3..=5, 3);
    c.fill(&K.log, [3, 6], 3..=5, 3);
    c.solid(&K.log_x, [4, 5, 3], [5, 5, 3]);
    c.fill(&wall, [1, 8], 3..=5, 4..=5);
    c.solid(&wall, [2, 3, 6], [7, 5, 6]);
    c.fill(&S.pane, [3, 6], 4, 6);
    c.door(DOOR, [4, 3, 3], North, Hinge::Left);
    c.door(DOOR, [5, 3, 3], North, Hinge::Right);
    eave(c, [0, 6, 2], [9, 7]);
    c.hip_roof(STAIRS, Some(&K.planks), [1, 7, 3], [8, 6], 1);
    turned_stairs(
        c,
        &[([1, 7, 3], East), ([8, 7, 3], South), ([1, 7, 6], North)],
    );

    c.solid(&block("minecraft:bookshelf"), [7, 3, 4], [7, 6, 4]);
    c.solid(&block("minecraft:bookshelf"), [2, 3, 5], [2, 6, 5]);
    c.lectern([7, 3, 5], West);
    c.place(&stairs(STAIRS, West), 2, 3, 4);
    checker(c, [&white, &orange], 3, [4, 4], [5, 5]);
    c.fill(&wall_torch(North), [3, 6], 5, 5);
    c.scatter(&S.short_grass, 1, tufts.as_chunks().0);
}

fn savanna_fletcher_house_1(c: &mut Canvas) {
    let wall = block("minecraft:yellow_terracotta");
    let tufts = [
        0, 0, 1, 0, 2, 0, 3, 0, 4, 0, 6, 0, 7, 0, 10, 0, 0, 1, 10, 1, 10, 2, 0, 3, 10, 3, 10, 4, 0,
        5, 10, 5, 0, 6, 2, 6, 4, 6, 5, 6, 6, 6, 10, 6, 0, 7, 6, 7, 8, 7, 1, 8, 2, 8, 8, 8, 10, 8,
    ];
    c.solid(&GROUND, [0, 0, 0], [10, 0, 8]);
    c.solid(&K.planks, [2, 0, 2], [8, 0, 4]);
    c.fill(&K.planks, [3, 7], 0, 5);
    c.fill(&S.path, [3, 7], 0, 6..=8);
    c.place(&S.grass, 4, 0, 8);

    c.fill(&wall, 2..=8, 1..=4, [1, 5]);
    c.fill(&wall, [1, 9], 1..=4, 2..=4);
    c.fill(&K.log, 5, 1..=4, [1, 5]);
    c.fill(&S.pane, [3, 7], 2..=3, 1);
    c.fill(&S.pane, [1, 9], 2..=3, 3);
    c.door(DOOR, [3, 1, 5], South, Hinge::Right);
    c.door(DOOR, [7, 1, 5], South, Hinge::Left);
    c.entrance([4, 1, 8], EMPTY, NOTHING);
    eave(c, [0, 5, 0], [10, 6]);
    c.place(&stairs(STAIRS, East), 0, 5, 0);
    c.place(&stairs(STAIRS, West), 10, 5, 0);
    c.fill(&stairs(STAIRS, North), [0, 10], 5, 6);
    c.hip_roof(STAIRS, None, [1, 6, 1], [9, 5], 1);
    c.solid(&K.planks, [2, 6, 2], [8, 6, 4]);
    c.place(&stairs(STAIRS, West), 9, 6, 5);

    c.posts(&K.fence, &[1, 5, 9], &[7], [1, 3]);
    c.fill(&slab("top"), [1, 9], 4, 5);
    c.solid(&slab("top"), [1, 4, 6], [9, 4, 6]);
    c.solid(&slab("bottom"), [1, 4, 7], [9, 4, 7]);
    c.fill(&slab("double"), [1, 5, 9], 4, 7);
    for x in [1, 5, 9] {
        banner(c, [x, 4, 8], South);
    }
    for x in [3, 7] {
        banner(c, [x, 4, 0], North);
    }
    banner(c, [0, 4, 3], West);
    banner(c, [10, 4, 3], East);

    for x in [2, 8] {
        c.place(&stairs(STAIRS, North), x, 1, 2);
        table(c, [x, 1, 3]);
        c.place(&stairs(STAIRS, South), x, 1, 4);
    }
    c.fill(&K.log_z, [4, 6], 1, 2);
    c.fill(&S.torch, [4, 6], 2, 2);
    c.place(&block("minecraft:fletching_table"), 5, 1, 2);
    c.fill(&S.wall_torch, [3, 7], 3, 6);
    c.place(&block("minecraft:poppy"), 0, 1, 2);
    c.scatter(&S.short_grass, 1, tufts.as_chunks().0);
}

fn savanna_cartographer_1(c: &mut Canvas) {
    c.fill(&K.fence, [2, 4, 6], 0..=1, [1, 7]);
    c.fill(&K.fence, [2, 6], 0..=1, [3, 5]);
    c.fill(&stairs(STAIRS, East), 0, 0, [3, 5]);
    c.solid(&K.planks, [1, 0, 3], [1, 0, 5]);
    c.solid(&stairs(STAIRS, East), [1, 1, 3], [1, 1, 5]);
    c.place(&K.planks, 2, 1, 4);
    c.entrance([0, 0, 4], EMPTY, STEP_EAST);
    c.solid(&slab("double"), [2, 2, 1], [6, 2, 7]);
    c.solid(&slab("bottom"), [2, 2, 2], [2, 2, 6]);

    c.fill(&K.wood, [2, 6], 3..=5, [1, 7]);
    c.fill(&K.log_z, 3..=5, 3..=5, [1, 7]);
    c.fill(&K.log_x, [3, 6], 3..=5, 2..=6);
    c.fill(&S.pane, 6, 4, [3, 5]);
    c.door(DOOR, [3, 3, 4], East, Hinge::Right);
    eave(c, [1, 6, 0], [7, 8]);
    c.solid(&K.planks, [3, 6, 2], [3, 6, 6]);
    c.fill(&stairs(STAIRS, East), 1, 6, [0, 8]);
    c.place(&stairs(STAIRS, West), 7, 6, 0);
    c.place(&stairs(STAIRS, North), 7, 6, 8);
    c.hip_roof(STAIRS, None, [2, 7, 1], [6, 7], 1);
    c.solid(&K.planks, [3, 7, 2], [5, 7, 6]);
    let turned = [
        ([2, 7, 1], East),
        ([6, 7, 1], South),
        ([2, 7, 7], North),
        ([6, 7, 7], West),
    ];
    turned_stairs(c, &turned);

    for x in 3..=5 {
        banner(c, [x, 3, 0], North);
        banner(c, [x, 3, 8], South);
    }
    for z in [3, 5] {
        banner(c, [2, 5, z], West);
    }
    c.place(&S.wall_torch, 2, 5, 4);
    c.place(&K.wood, 4, 3, 2);
    c.place(&S.torch, 4, 4, 2);
    c.place(&block("minecraft:cartography_table"), 5, 3, 2);
    chest(c, [4, 3, 6], North, "village_cartographer");
    c.place(&K.wood, 5, 3, 6);
    c.place(&S.torch, 5, 4, 6);
    c.spot([4, 0, 4], CATS, NOTHING);
}

fn savanna_butchers_shop_1(c: &mut Canvas) {
    let yellow = block("minecraft:yellow_terracotta");
    let orange = block("minecraft:orange_terracotta");
    let smooth = |kind: &str| {
        block(&format!(
            "minecraft:smooth_stone_slab[type={kind},waterlogged=false]"
        ))
    };
    let chimney = |height: &str, sides: &[Direction]| {
        let joined = |side: Direction| {
            if sides.contains(&side) {
                height
            } else {
                "none"
            }
        };
        block(&format!(
            "minecraft:cobblestone_wall[east={},north={},south={},up=true,waterlogged=false,west={}]",
            joined(East),
            joined(North),
            joined(South),
            joined(West)
        ))
    };
    let tufts = [
        2, 0, 4, 0, 5, 0, 8, 0, 10, 0, 2, 1, 3, 1, 6, 1, 7, 1, 8, 1, 9, 1, 0, 2, 1, 2, 1, 6, 1, 8,
        7, 8, 9, 8, 10, 8, 1, 9, 6, 9, 1, 10, 3, 10, 4, 10, 5, 10, 7, 10,
    ];
    let (east, west) = (stairs(STAIRS, East), stairs(STAIRS, West));
    let (north, south) = (stairs(STAIRS, North), stairs(STAIRS, South));

    c.scatter(&S.short_grass, 0, tufts.as_chunks().0);
    checker(c, [&yellow, &orange], 0, [2, 3], [5, 4]);
    checker(c, [&yellow, &orange], 0, [4, 2], [5, 8]);
    c.place(&yellow, 3, 0, 5);
    c.solid(&smooth("top"), [4, 0, 7], [5, 0, 7]);
    c.place(&south, 0, 0, 3);
    c.place(&east, 0, 0, 4);
    c.place(&north, 0, 0, 5);
    c.entrance([0, 0, 6], EMPTY, NOTHING);
    c.walls(&K.log, &K.log, [6, 0, 2], [10, 0, 7]);
    c.solid(&S.grass, [7, 0, 3], [9, 0, 6]);
    c.spot([8, 0, 5], BUTCHER_ANIMALS, DIRT);

    let posts = [
        4, 1, 5, 1, 2, 2, 3, 2, 6, 2, 1, 3, 1, 5, 2, 5, 3, 6, 3, 8, 6, 8, 4, 9, 5, 9, 6, 4, 6, 6,
    ];
    for y in 0..=3 {
        c.scatter(&K.log, y, posts.as_chunks().0);
    }
    c.place(&K.log, 3, 0, 7);
    c.place(&K.log_z, 1, 0, 4);
    for [x, z] in [[3, 7], [6, 3], [6, 7]] {
        c.fill(&K.log_z, x, [1, 3], z);
        c.place(&S.pane, x, 2, z);
    }
    c.each(&K.log_z, &[[1, 3, 4], [6, 3, 5]]);
    c.door(DOOR, [1, 1, 4], West, Hinge::Left);
    c.open_door("minecraft:acacia_door", [6, 1, 5], South, Hinge::Right);

    c.fill(&south, 2..=6, 4, 0);
    c.fill(&north, 3..=6, 4, 10);
    c.fill(&east, 0, 4, 1..=6);
    c.fill(&east, 2, 4, [1, 7, 8, 9]);
    c.fill(&west, 7, 4, 1..=9);
    c.each(&south, &[[1, 4, 1]]);
    c.each(&north, &[[1, 4, 6]]);
    c.fill(&K.planks, 3..=6, 4, [1, 9]);
    c.fill(&K.planks, 6, 4, 2..=8);
    c.fill(&K.planks, 1, 4, 2..=5);
    c.fill(&K.planks, 2..=3, 4, 2);
    c.each(&K.planks, &[[2, 4, 5], [2, 4, 6]]);
    c.fill(&K.planks, 3, 4, 6..=8);

    c.fill(&south, 4..=6, 5, 1);
    c.fill(&south, 2..=3, 5, 2);
    c.fill(&east, 1, 5, 2..=4);
    c.fill(&east, 3, 5, [1, 6, 7, 8, 9]);
    c.place(&east, 2, 5, 5);
    c.each(&north, &[[1, 5, 5], [2, 5, 6]]);
    c.fill(&north, 4..=5, 5, 9);
    c.fill(&west, 6, 5, 2..=9);
    c.solid(&K.planks, [2, 5, 3], [5, 5, 4]);
    c.solid(&K.planks, [3, 5, 5], [5, 5, 5]);
    c.solid(&K.planks, [4, 5, 6], [5, 5, 8]);
    c.place(&K.planks, 5, 5, 2);

    c.furnace([4, 1, 2], "smoker", South);
    c.solid(&chimney("tall", &[North, West]), [4, 2, 2], [4, 4, 2]);
    c.place(&chimney("low", &[East, North, South, West]), 4, 5, 2);
    c.place(&chimney("low", &[]), 4, 6, 2);
    c.place(&block("minecraft:cobblestone"), 5, 1, 2);
    c.solid(&smooth("double"), [4, 1, 8], [5, 1, 8]);
    c.fill(&S.wall_torch, 0, 2, [3, 5]);
    c.place(&wall_torch(East), 2, 3, 4);
    c.place(&S.wall_torch, 5, 3, 5);
    c.solid(&K.fence, [7, 1, 2], [10, 1, 2]);
    c.solid(&K.fence, [7, 1, 7], [10, 1, 7]);
    c.solid(&K.fence, [10, 1, 3], [10, 1, 6]);
    c.fill(&S.torch, 10, 2, [2, 7]);
}

fn savanna_butchers_shop_2(c: &mut Canvas) {
    let tufts = [
        1, 0, 0, 1, 2, 1, 3, 1, 5, 1, 7, 1, 8, 1, 9, 1, 10, 1, 0, 2, 11, 2, 5, 3, 9, 3, 10, 3, 11,
        3, 8, 4, 10, 4, 6, 5, 9, 5, 10, 5, 11, 5, 0, 6, 5, 6, 6, 6, 11, 6, 5, 7, 6, 7, 8, 7, 9, 7,
        10, 7,
    ];
    let east = stairs(STAIRS, East);
    c.void([0, 1, 0], [12, 9, 8]);
    c.solid(&S.grass, [0, 0, 0], [12, 0, 8]);
    c.solid(&S.dirt, [0, 0, 3], [1, 0, 5]);
    c.place(&S.dirt, 10, 0, 6);
    c.solid(&S.air, [1, 0, 7], [3, 0, 7]);
    c.place(&S.air, 0, 0, 8);
    c.each(&S.short_grass, &[[0, 0, 7], [1, 0, 8], [2, 0, 8]]);
    c.solid(&S.air, [1, 1, 7], [3, 1, 7]);

    c.fill(&K.fence, 4..=12, 1, [0, 8]);
    c.solid(&K.fence, [12, 1, 1], [12, 1, 7]);
    c.fill(&K.fence, 4, 1, [1, 7]);
    c.fill(&K.fence, 1..=4, 1, [2, 6]);
    c.posts(&K.fence, &[4, 7], &[2, 4, 6], [1, 2]);
    c.solid(&K.fence, [8, 1, 5], [8, 2, 5]);
    c.place(&S.torch, 5, 1, 4);
    c.each(&S.torch, &[[12, 2, 0], [4, 2, 8], [12, 2, 8]]);
    c.scatter(&S.short_grass, 1, tufts.as_chunks().0);

    c.fill(&east, 0, 1, [3, 5]);
    c.entrance([0, 1, 4], EMPTY, STEP_EAST);
    c.solid(&K.planks, [1, 1, 3], [1, 1, 5]);
    c.solid(&east, [1, 2, 3], [1, 2, 5]);
    c.solid(&slab("top"), [2, 2, 3], [2, 2, 5]);
    c.solid(&east, [2, 3, 3], [2, 3, 5]);
    c.place(&stairs(STAIRS, West), 10, 1, 6);
    c.place(&slab("top"), 9, 1, 6);
    c.place(&stairs(STAIRS, West), 9, 2, 6);
    c.place(&slab("top"), 8, 2, 6);
    c.place(&stairs(STAIRS, North), 8, 3, 5);
    c.solid(&slab("top"), [8, 3, 3], [8, 3, 4]);
    c.solid(&K.planks, [3, 3, 3], [7, 3, 5]);
    c.fill(&K.planks, 4..=7, 3, [2, 6]);
    c.fill(&K.wood_x, [4, 7], 3, 4);

    c.fill(&K.wood, [4, 7], 4..=6, [2, 3, 5, 6]);
    c.fill(&K.wood_x, 5..=6, [4, 6], [2, 6]);
    c.fill(&S.pane, 5..=6, 5, [2, 6]);
    c.fill(&K.wood_z, [4, 7], 6, 4);
    c.door(DOOR, [4, 4, 4], West, Hinge::Left);
    c.door(DOOR, [7, 4, 4], East, Hinge::Right);
    eave(c, [3, 7, 1], [8, 7]);
    c.hip_roof(STAIRS, Some(&K.planks), [4, 8, 2], [7, 6], 1);
    turned_stairs(c, &[([4, 8, 2], East), ([4, 8, 6], North)]);

    c.solid(&stairs(STAIRS, North), [5, 4, 3], [6, 4, 3]);
    chest(c, [6, 4, 5], North, "village_butcher");
    c.furnace([8, 4, 3], "smoker", East);
    c.fill(&S.wall_torch, 3, 5, [3, 5]);
    c.fill(&S.wall_torch, [6, 8], 6, 4);
    c.spot([10, 0, 2], BUTCHER_ANIMALS, GRASS);
}

fn savanna_tool_smith_1(c: &mut Canvas) {
    let tufts = [
        0, 0, 3, 1, 4, 2, 5, 2, 0, 3, 1, 3, 6, 4, 6, 5, 6, 6, 0, 7, 1, 7, 5, 7, 6, 7, 0, 8, 2, 8,
        3, 8, 4, 8, 5, 8, 0, 9, 1, 9, 5, 9, 1, 10, 3, 10, 4, 10, 5, 10, 6, 10,
    ];
    c.solid(&GROUND, [0, 0, 0], [6, 0, 10]);
    c.solid(&K.planks, [2, 0, 4], [4, 0, 6]);
    c.fill(&K.planks, 3, 0, [3, 7]);
    c.place(&K.planks, 1, 0, 5);
    c.place(&S.path, 0, 0, 5);

    round_room(c, [0, 0, 2], &K.log, [&K.log_x, &K.log_z]);
    round_roof(c, [0, 4, 2], &[([1, 5, 3], East), ([2, 6, 4], East)]);
    c.door(DOOR, [3, 1, 3], North, Hinge::Left);
    c.door(DOOR, [1, 1, 5], West, Hinge::Right);
    c.door(DOOR, [3, 1, 7], South, Hinge::Left);
    c.entrance([0, 1, 4], EMPTY, NOTHING);

    for (post, inner, edge, out) in [(1, 2, 0, North), (9, 8, 10, South)] {
        c.posts(&K.fence, &[2, 4], &[post], [1, 3]);
        c.fill(&K.planks, 2..=4, 4, [post, inner]);
        c.place(&slab("top"), 3, 4, inner);
        for x in 2..=4 {
            banner(c, [x, 4, edge], out);
        }
        for z in [post, inner] {
            banner(c, [1, 4, z], West);
            banner(c, [5, 4, z], East);
        }
    }

    c.fill(&stairs(STAIRS, East), 4, 1, [4, 6]);
    c.place(&block("minecraft:smithing_table"), 4, 1, 5);
    c.fill(&S.wall_torch, [0, 4], 3, 5);
    c.solid(&K.cave_air, [4, 1, 0], [5, 1, 0]);
    c.scatter(&S.short_grass, 1, tufts.as_chunks().0);
}

fn savanna_weaponsmith_2(c: &mut Canvas) {
    let stone = block("minecraft:smooth_stone");
    let rim = block("minecraft:smooth_stone_slab[type=bottom,waterlogged=false]");
    let bars = settled("minecraft:iron_bars[waterlogged=false]");
    let (east, west) = (stairs(STAIRS, East), stairs(STAIRS, West));
    let (north, south) = (stairs(STAIRS, North), stairs(STAIRS, South));
    c.void([0, 0, 0], [8, 6, 12]);

    c.solid(&S.dirt, [2, 0, 2], [7, 0, 10]);
    c.solid(&S.dirt, [1, 0, 5], [1, 0, 10]);
    c.solid(&S.dirt, [5, 0, 1], [6, 0, 1]);
    c.solid(&S.dirt, [2, 0, 11], [6, 0, 11]);
    c.place(&S.dirt, 0, 0, 6);
    for (y, cell) in [(0, &S.grass), (1, &rim)] {
        c.fill(cell, 0, y, [4, 5, 7, 8]);
        c.fill(cell, 1, y, [4, 8]);
    }
    c.solid(&S.short_grass, [8, 0, 0], [8, 0, 3]);
    c.place(&K.cave_air, 8, 0, 4);
    c.solid(&K.cave_air, [0, 1, 0], [2, 1, 0]);

    c.solid(&stone, [2, 1, 2], [6, 1, 4]);
    c.solid(&stone, [1, 1, 5], [3, 1, 7]);
    c.walls(&stone, &stone, [2, 2, 2], [4, 2, 4]);
    c.place(&block("minecraft:lava[level=0]"), 3, 2, 3);
    c.solid(&stone, [4, 3, 2], [4, 5, 5]);
    c.solid(&stone, [5, 3, 1], [5, 5, 1]);
    c.place(&stone, 4, 5, 1);
    for y in [3, 4] {
        c.solid(&bars, [2, y, 2], [3, y, 2]);
        c.solid(&bars, [2, y, 3], [2, y, 4]);
    }
    c.entrance([0, 1, 6], EMPTY, "minecraft:smooth_stone_slab[type=bottom]");
    c.solid(&rim, [2, 5, 2], [3, 5, 4]);
    c.each(&rim, &[[4, 5, 0], [5, 5, 0], [3, 5, 1], [3, 5, 5]]);
    c.each(
        &S.air,
        &[[3, 3, 5], [1, 3, 8], [5, 5, 3], [2, 5, 5], [5, 5, 5]],
    );

    c.solid(&K.planks, [5, 1, 5], [6, 1, 10]);
    c.solid(&K.planks, [2, 1, 9], [4, 1, 10]);
    c.place(&K.planks, 4, 1, 8);
    c.solid(&K.wood, [7, 1, 2], [7, 4, 10]);
    c.fill(&K.wood, 5..=6, 1..=2, 1);
    c.solid(&K.wood, [6, 3, 1], [6, 4, 1]);
    c.solid(&K.wood, [1, 1, 9], [1, 4, 10]);
    c.solid(&K.wood, [2, 1, 8], [3, 4, 8]);
    c.solid(&K.wood, [2, 1, 11], [6, 4, 11]);
    c.solid(&K.wood, [4, 1, 5], [4, 2, 7]);
    c.solid(&K.log, [4, 3, 7], [4, 4, 7]);
    c.place(&K.log_z, 4, 4, 6);
    c.place(&K.wood_z, 4, 1, 7);
    c.place(&K.wood_x, 3, 1, 8);
    c.solid(&K.wood_x, [7, 1, 8], [7, 1, 9]);
    c.place(&K.wood_x, 7, 2, 3);
    c.fill(&K.wood_z, 7, 3, [3, 5, 6, 7, 9]);
    c.fill(&S.pane, 7, 3, [4, 8]);
    c.fill(&K.wood_x, [3, 5, 6], 3, 11);
    c.place(&S.pane, 4, 3, 11);
    c.door(DOOR, [4, 2, 6], West, Hinge::Left);

    c.fill(&south, 6..=7, 5, 0);
    c.fill(&K.planks, 6..=7, 5, 1);
    c.solid(&K.planks, [7, 5, 2], [7, 5, 11]);
    c.solid(&west, [8, 5, 1], [8, 5, 11]);
    c.solid(&K.planks, [4, 5, 6], [4, 5, 8]);
    c.fill(&east, 3, 5, [6, 7]);
    c.fill(&south, 1..=2, 5, 7);
    c.solid(&east, [0, 5, 8], [0, 5, 11]);
    c.solid(&K.planks, [1, 5, 8], [1, 5, 11]);
    c.solid(&K.planks, [2, 5, 8], [3, 5, 8]);
    c.solid(&K.planks, [2, 5, 11], [6, 5, 11]);
    c.fill(&north, 1..=7, 5, 12);
    c.fill(&south, 4..=7, 6, 1);
    c.solid(&east, [4, 6, 2], [4, 6, 8]);
    c.solid(&K.planks, [5, 6, 2], [6, 6, 10]);
    c.solid(&west, [7, 6, 2], [7, 6, 10]);
    c.fill(&south, 1..=3, 6, 8);
    c.fill(&east, 1, 6, [9, 10]);
    c.solid(&K.planks, [2, 6, 9], [4, 6, 10]);
    c.fill(&north, 1..=7, 6, 11);

    c.place(
        &block("minecraft:grindstone[face=wall,facing=east]"),
        5,
        2,
        3,
    );
    c.place(
        &block("minecraft:grindstone[face=floor,facing=east]"),
        2,
        2,
        6,
    );
    chest(c, [2, 2, 9], East, "village_weaponsmith");
    c.place(&north, 6, 2, 8);
    table(c, [6, 2, 9]);
    c.place(&south, 6, 2, 10);
    c.place(&wall_torch(West), 1, 3, 11);
    c.each(&wall_torch(West), &[[6, 4, 4], [3, 4, 6], [6, 4, 8]]);
    c.place(&wall_torch(East), 2, 4, 9);
}

fn savanna_shepherd_1(c: &mut Canvas) {
    let tufts = [
        9, 1, 10, 1, 7, 2, 8, 2, 10, 2, 6, 3, 7, 3, 8, 4, 7, 5, 8, 5, 11, 5, 8, 6, 11, 6, 5, 7, 6,
        7, 7, 7, 8, 7, 9, 7, 10, 7, 7, 8, 8, 8, 11, 8, 5, 9, 7, 9, 8, 9, 10, 9, 11, 9,
    ];
    c.solid(&GROUND, [0, 0, 0], [12, 1, 10]);
    c.each(&S.air, &[[3, 0, 0], [0, 1, 0], [1, 1, 0], [0, 1, 1]]);
    c.place(&S.grass, 11, 0, 4);
    c.runs(
        &S.water,
        1,
        &[[9, 9, 2], [8, 10, 3], [9, 11, 4], [10, 10, 5]],
    );
    c.solid(&K.planks, [2, 1, 4], [4, 1, 6]);
    c.place(&K.log, 3, 1, 5);
    c.fill(&K.log_x, [1, 5], 1, 5);
    c.place(&S.path, 0, 1, 5);
    c.place(&S.dirt, 0, 0, 5);

    round_room(c, [0, 1, 2], &K.log, [&K.log_x, &K.log_z]);
    let turned = [
        ([1, 6, 3], East),
        ([5, 6, 3], South),
        ([1, 6, 7], North),
        ([4, 7, 4], South),
    ];
    round_roof(c, [0, 5, 2], &turned);
    c.door(DOOR, [1, 2, 5], West, Hinge::Right);
    c.door(DOOR, [5, 2, 5], East, Hinge::Left);
    c.entrance([0, 2, 5], EMPTY, AIR);
    c.fill(&K.wood, [2, 4], 2, 6);
    c.place(&block("minecraft:loom[facing=south]"), 3, 2, 6);
    c.fill(&S.wall_torch, 0, 3, [4, 6]);
    c.place(&S.wall_torch, 4, 4, 5);

    c.fill(&K.fence, 4..=12, 2, [0, 10]);
    c.solid(&K.fence, [12, 2, 1], [12, 2, 9]);
    c.fill(&K.fence, 4, 2, [1, 2, 8, 9]);
    c.each(&S.torch, &[[12, 3, 6], [8, 3, 10], [12, 3, 10]]);
    c.scatter(&S.short_grass, 2, tufts.as_chunks().0);
    c.place(&S.tall_grass, 11, 2, 2);
    c.spot([7, 1, 4], SHEEP, GRASS);
    c.spot([10, 1, 8], TREES, DIRT);
}

fn savanna_fisher_cottage_1(c: &mut Canvas) {
    let shore = [
        2, 4, 0, 2, 4, 1, 2, 6, 2, 1, 7, 3, 1, 7, 4, 0, 7, 5, 0, 7, 6, 0, 7, 7, 0, 7, 8,
    ];
    c.void([0, 0, 0], [7, 1, 8]);
    c.runs(&S.dirt, 0, shore.as_chunks().0);
    c.runs(&GROUND, 1, shore.as_chunks().0);
    c.place(&S.air, 0, 1, 2);
    c.runs(&S.water, 1, &[[4, 6, 4], [3, 6, 5], [2, 6, 6], [2, 6, 7]]);
    c.fill(
        &settled("minecraft:acacia_fence[waterlogged=true]"),
        [3, 5],
        1,
        6,
    );

    for step in 0..3 {
        c.solid(
            &stairs(STAIRS, South),
            [2, 2 + step, step],
            [4, 2 + step, step],
        );
        c.solid(&K.planks, [2, 2 + step, step + 1], [4, 2 + step, step + 1]);
    }
    c.entrance([3, 2, 0], EMPTY, "minecraft:acacia_stairs[facing=south]");
    c.posts(&K.fence, &[1, 3, 5], &[3, 6], [2, 4]);
    c.solid(&slab("double"), [1, 5, 3], [5, 5, 6]);
    c.solid(&slab("bottom"), [2, 5, 3], [4, 5, 3]);

    c.fill(&K.wood, [1, 5], 6..=8, [3, 6]);
    c.fill(&K.log_x, [1, 5], 6..=8, 4..=5);
    c.fill(&K.log_z, 2..=4, 6..=8, [4, 6]);
    c.place(&S.pane, 3, 7, 6);
    c.door(DOOR, [3, 6, 4], South, Hinge::Left);
    eave(c, [0, 9, 2], [6, 7]);
    c.solid(&K.planks, [2, 9, 4], [4, 9, 4]);
    c.place(&stairs(STAIRS, South), 0, 9, 2);
    c.place(&stairs(STAIRS, West), 6, 9, 2);
    c.place(&stairs(STAIRS, East), 0, 9, 7);
    c.place(&stairs(STAIRS, North), 6, 9, 7);
    c.hip_roof(STAIRS, Some(&K.planks), [1, 10, 3], [5, 6], 1);
    let turned = [
        ([1, 10, 3], East),
        ([5, 10, 3], South),
        ([1, 10, 6], North),
        ([5, 10, 6], West),
    ];
    turned_stairs(c, &turned);

    c.barrel([2, 2, 4], East);
    c.barrel([2, 3, 4], Direction::Up);
    c.barrel([6, 2, 2], North);
    c.fill(&wall_torch(North), [2, 4], 7, 3);
    c.place(&wall_torch(South), 3, 8, 5);
    c.scatter(
        &S.short_grass,
        2,
        &[[2, 2], [6, 3], [7, 3], [0, 6], [1, 7], [2, 8]],
    );
    spots(c, CATS, GRASS, &[[4, 1, 3], [0, 1, 7], [6, 1, 8]]);
}

/// Four fence posts under a roof of two rows of stairs between slab eaves,
/// over a counter of two logs; the ridge runs along `along`.
fn stall(c: &mut Canvas, [x, y, z]: [i32; 3], along: Axis) {
    let place = |c: &mut Canvas, cell: &Cell, across: i32, dy: i32, ridge: i32| {
        if along == Z {
            c.place(cell, x + across, y + dy, z + ridge);
        } else {
            c.place(cell, x + ridge, y + dy, z + across);
        }
    };
    let (low, high) = if along == Z {
        (East, West)
    } else {
        (South, North)
    };
    let roof = [
        slab("bottom"),
        stairs(STAIRS, low),
        stairs(STAIRS, high),
        slab("bottom"),
    ];
    for ridge in 0..3 {
        for (across, cell) in (0..).zip(&roof) {
            place(c, cell, across, 3, ridge);
        }
    }
    for across in [0, 3] {
        for dy in 0..3 {
            place(c, &K.fence, across, dy, 0);
            place(c, &K.fence, across, dy, 2);
        }
    }
    for across in [1, 2] {
        place(c, &K.log, across, 0, 1);
    }
}

fn meeting_point_1(c: &mut Canvas, v: Village) {
    let path = [
        0, 4, 0, 6, 6, 0, 8, 8, 0, 10, 10, 0, 6, 6, 1, 8, 8, 1, 10, 10, 1, 12, 12, 1, 0, 1, 2, 5,
        5, 2, 7, 12, 2, 0, 0, 3, 2, 3, 3, 6, 6, 3, 8, 10, 3, 12, 12, 3, 0, 5, 4, 7, 8, 4, 0, 1, 5,
        3, 5, 5, 7, 7, 5, 9, 10, 5, 0, 2, 6, 4, 4, 6, 8, 10, 6, 1, 9, 7, 11, 11, 7, 0, 0, 8, 2, 3,
        8, 5, 5, 8, 8, 10, 8, 5, 7, 9, 9, 9, 9, 11, 11, 9, 0, 0, 10, 2, 3, 10, 5, 5, 10, 7, 10, 10,
        1, 2, 11, 5, 6, 11, 9, 9, 11,
    ];
    let tufts = [
        9, 0, 11, 0, 0, 1, 2, 1, 5, 1, 9, 1, 11, 1, 4, 2, 6, 2, 5, 3, 9, 4, 2, 5, 8, 5, 5, 6, 7, 6,
        0, 7, 6, 8, 7, 8, 0, 9, 1, 9, 4, 9, 10, 9, 6, 10, 3, 11, 4, 11,
    ];
    let hollow = [
        9, 9, 12, 9, 7, 10, 8, 10, 9, 10, 10, 10, 12, 10, 6, 11, 7, 11, 9, 11, 10, 11, 11, 11, 12,
        11, 13, 11,
    ];
    c.solid(&GROUND, [0, 0, 0], [13, 0, 11]);
    c.runs(&S.path, 0, path.as_chunks().0);
    c.scatter(&S.short_grass, 1, tufts.as_chunks().0);
    c.place(&S.tall_grass, 13, 1, 5);
    c.scatter(&K.cave_air, 1, hollow.as_chunks().0);
    c.runs(&K.cave_air, 2, &[[7, 10, 10], [7, 12, 11]]);

    for (z, edge, out) in [(1, 0, North), (8, 11, South)] {
        stall(c, [1, 1, z], Z);
        for x in [2, 3] {
            banner(c, [x, 4, edge], out);
        }
    }
    stall(c, [10, 1, 4], X);
    for z in [5, 6] {
        banner(c, [13, 4, z], East);
    }
    for [x, z] in [[5, 0], [0, 11], [11, 10]] {
        c.torch_post(&K.fence, [x, 1, z], 1);
    }
    c.bell([6, 1, 6], "floor", East);
    street_ends(c, v, &[[7, 0], [0, 5], [8, 11]]);
    if v.zombie {
        c.void([0, 5, 0], [13, 5, 11]);
    } else {
        c.each(&S.path, &[[7, 0, 0], [8, 0, 11]]);
        spots(c, CATS, PATH, &[[2, 0, 4], [8, 0, 6]]);
        c.spot([8, 0, 9], CATS, GRASS);
        c.spot([3, 0, 6], IRON_GOLEM, GRASS);
        villagers(c, v, PATH, &[[5, 0, 4], [8, 0, 4], [10, 0, 2]]);
    }
}

fn meeting_point_2(c: &mut Canvas, v: Village) {
    let orange = block("minecraft:orange_terracotta");
    let yellow = block("minecraft:yellow_terracotta");
    let path = [
        4, 6, 0, 4, 6, 1, 3, 7, 2, 2, 3, 3, 7, 8, 3, 0, 2, 4, 8, 10, 4, 0, 2, 5, 8, 10, 5, 0, 2, 6,
        8, 10, 6, 2, 3, 7, 7, 8, 7, 3, 7, 8, 4, 6, 9, 4, 6, 10,
    ];
    let lawn = [
        7, 8, 1, 8, 9, 2, 9, 10, 3, 0, 1, 7, 9, 10, 7, 2, 2, 8, 8, 9, 8, 2, 3, 9, 7, 8, 9, 3, 3,
        10, 7, 7, 10,
    ];
    let overgrowth = [
        7, 10, 0, 10, 10, 1, 10, 10, 2, 0, 0, 8, 10, 10, 8, 0, 0, 9, 10, 10, 9, 0, 2, 10, 8, 10, 10,
    ];
    c.void([0, 0, 0], [10, 0, 10]);
    c.runs(&S.path, 0, path.as_chunks().0);
    c.runs(&S.grass, 0, lawn.as_chunks().0);
    c.solid(&S.grass, [4, 0, 4], [6, 0, 6]);
    footing(c, [2, 2]);
    c.fill(&S.dirt, [1, 9], 0, [1, 9]);
    c.each(
        if v.zombie { &S.dirt } else { &S.path },
        &[[1, 0, 9], [5, 0, 5]],
    );
    if v.zombie {
        c.runs(&S.grass, 0, overgrowth.as_chunks().0);
        c.scatter(&S.short_grass, 1, &[[10, 1], [0, 8], [0, 9]]);
    } else {
        c.void([0, 1, 0], [10, 5, 10]);
        c.place(&S.air, 1, 1, 6);
        c.spot([9, 0, 7], IRON_GOLEM, GRASS);
        villagers(c, v, PATH, &[[1, 0, 6], [7, 0, 8], [4, 0, 9]]);
    }
    spots(c, CATS, GRASS, &[[1, 0, 3], [1, 0, 8]]);

    round_walls(c, &orange, [2, 2], 1);
    wall_middles(c, [2, 1, 2], [&yellow, &yellow]);
    c.solid(&S.water, [4, 1, 4], [6, 1, 6]);
    c.solid(&yellow, [5, 1, 5], [5, 5, 5]);
    c.place(&orange, 5, 4, 5);
    banner(c, [4, 4, 5], West);
    banner(c, [6, 4, 5], East);
    banner(c, [5, 4, 4], North);
    banner(c, [5, 4, 6], South);
    c.fill(&yellow, [1, 9], 1, [1, 9]);
    c.bell([1, 2, 1], "floor", East);
    c.bell([9, 2, 9], "floor", West);
    c.each(&S.torch, &[[9, 2, 1], [1, 2, 9]]);
    c.scatter(
        &S.short_grass,
        1,
        &[[0, 7], [1, 7], [2, 8], [2, 9], [3, 10]],
    );
    street_ends(c, v, &[[5, 0], [0, 5], [10, 5], [5, 10]]);
}

fn meeting_point_3(c: &mut Canvas, v: Village) {
    c.void([0, 0, 0], [8, 5, 10]);
    c.rows(&S.path, 0, 0, &[[3, 5], [2, 6], [2, 6], [2, 7]]);
    c.solid(&S.path, [0, 0, 4], [8, 0, 6]);
    c.rows(&S.path, 0, 7, &[[2, 7], [2, 6], [2, 6], [3, 5]]);
    c.place(&K.wood, 4, 0, 3);
    c.place(&K.wood_x, 4, 0, 7);
    for z in [2, 6] {
        c.solid(&K.wood, [3, 1, z], [5, 1, z + 2]);
        c.fill(&K.wood_x, 4, 1, [z, z + 2]);
        c.place(&S.water, 4, 1, z + 1);
    }
    c.posts(&K.fence, &[3, 5], &[4, 6], [2, 3]);
    c.bell([4, 3, 5], "ceiling", East);
    c.walls(&slab("bottom"), &slab("bottom"), [3, 4, 4], [5, 4, 6]);
    c.place(&slab("double"), 4, 4, 5);
    c.place(&S.torch, 4, 5, 5);
    street_ends(c, v, &[[4, 0], [0, 5], [8, 5], [4, 10]]);
    spots(c, CATS, GRASS, &[[7, 0, 1], [1, 0, 2], [8, 0, 9]]);
    if v.zombie {
        c.each(&S.path, &[[1, 0, 3], [2, 0, 5], [1, 0, 7]]);
    } else {
        c.each(&S.air, &[[0, 3, 0], [1, 1, 7]]);
        c.fill(&S.torch, [3, 5], 2, [2, 8]);
        villagers(c, v, PATH, &[[1, 0, 3], [2, 0, 5]]);
        c.spot([1, 0, 7], IRON_GOLEM, PATH);
    }
}

fn meeting_point_4(c: &mut Canvas, v: Village) {
    c.void([0, 0, 0], [8, 5, 8]);
    c.solid(&S.air, [0, 0, 0], [8, 5, 0]);
    c.solid(&S.path, [3, 0, 0], [5, 0, 0]);
    c.solid(&S.path, [1, 0, 1], [7, 0, 7]);
    c.solid(&S.path, [0, 0, 3], [8, 0, 5]);
    c.solid(&S.path, [3, 0, 8], [5, 0, 8]);
    c.patch(&K.wood, 0, &Patch::diamond([4, 4], 1));

    c.patch(&K.wood, 1, &Patch::clipped_rectangle([2, 2], [6, 6], 1));
    c.patch(&S.water, 1, &Patch::diamond([4, 4], 1));
    c.place(&K.wood_z, 2, 1, 5);
    c.fill(&S.torch, 4, 2, [2, 6]);
    c.fill(&S.torch, [2, 6], 2, 4);
    c.posts(&K.fence, &[3, 5], &[3, 5], [2, 3]);
    c.hip_roof(STAIRS, Some(&slab("double")), [3, 4, 3], [5, 5], 1);
    turned_stairs(
        c,
        &[([3, 4, 3], East), ([5, 4, 3], South), ([5, 4, 5], West)],
    );
    street_ends(c, v, &[[4, 0], [0, 4], [8, 4], [4, 8]]);
    spots(c, CATS, GRASS, &[[8, 0, 0], [7, 0, 8]]);
    if v.zombie {
        c.place(&K.wood, 1, 0, 7);
        c.bell([1, 1, 7], "floor", East);
    } else {
        c.fill(&S.air, 1, 1..=2, 1);
        c.fill(&S.air, 0, 1..=2, 7);
        c.place(&S.air, 1, 1, 7);
        c.bell([4, 3, 4], "ceiling", East);
        villagers(c, v, PATH, &[[1, 0, 1], [0, 0, 3]]);
        c.spot([1, 0, 7], IRON_GOLEM, PATH);
    }
}

fn savanna_lamp_post_01(c: &mut Canvas) {
    c.socket_facing([0, 0, 0], "down_south", BOTTOM, EMPTY, FENCE);
    c.place(&S.torch, 0, 1, 0);
}

#[rustfmt::skip]
mod data {
    use super::{Fields, Tag};

    pub const BANNER_DATA: Fields = &[("patterns", Tag::List(&[])), ("id", Tag::String("minecraft:banner"))];
}
use data::*;

templates! {
    "savanna" "savanna";
    both {
        "houses/savanna_animal_pen_2" [13, 7, 12] animal_pen_2;
        "houses/savanna_animal_pen_3" [8, 5, 9] animal_pen_3;
        "houses/savanna_large_farm_2" [10, 7, 8] large_farm_2;
        "houses/savanna_medium_house_1" [8, 7, 15] medium_house_1;
        "houses/savanna_medium_house_2" [10, 8, 11] medium_house_2;
        "houses/savanna_small_house_1" [7, 7, 7] small_house_1;
        "houses/savanna_small_house_2" [7, 7, 7] small_house_2;
        "houses/savanna_small_house_3" [7, 7, 7] small_house_3;
        "houses/savanna_small_house_4" [10, 8, 7] small_house_4;
        "houses/savanna_small_house_5" [7, 10, 7] small_house_5;
        "houses/savanna_small_house_6" [7, 7, 7] small_house_6;
        "houses/savanna_small_house_7" [7, 7, 7] small_house_7;
        "houses/savanna_small_house_8" [6, 7, 7] small_house_8;
        "streets/corner_01" [16, 2, 16] corner_01_of;
        "streets/corner_03" [4, 2, 4] corner_03_of;
        "streets/crossroad_02" [7, 2, 10] crossroad_02_of;
        "streets/crossroad_03" [16, 2, 16] crossroad_03_of;
        "streets/crossroad_04" [4, 2, 5] village_plains::crossroad_04_of;
        "streets/crossroad_05" [5, 2, 5] crossroad_05_of;
        "streets/crossroad_06" [5, 2, 5] crossroad_06_of;
        "streets/crossroad_07" [15, 2, 14] crossroad_07_of;
        "streets/split_01" [8, 2, 6] split_01_of;
        "streets/split_02" [14, 2, 9] split_02_of;
        "streets/straight_02" [16, 2, 16] straight_02_of;
        "streets/straight_04" [11, 2, 9] straight_04_of;
        "streets/straight_05" [20, 2, 17] straight_05_of;
        "streets/straight_06" [9, 2, 14] straight_06_of;
        "streets/straight_08" [10, 2, 16] straight_08_of;
        "streets/straight_09" [23, 2, 16] straight_09_of;
        "streets/straight_10" [6, 2, 11] straight_10_of;
        "streets/straight_11" [17, 2, 17] straight_11_of;
        "streets/turn_01" [19, 2, 11] turn_01_of;
        "terminators/terminator_05" [16, 2, 16] terminator_05_of;
        "town_centers/savanna_meeting_point_2" [11, 6, 11] meeting_point_2;
        "town_centers/savanna_meeting_point_3" [9, 6, 11] meeting_point_3;
        "town_centers/savanna_meeting_point_4" [9, 6, 9] meeting_point_4;
    }
    single {
        "houses/savanna_animal_pen_1" [9, 5, 9] savanna_animal_pen_1;
        "houses/savanna_armorer_1" [7, 7, 7] savanna_armorer_1;
        "houses/savanna_butchers_shop_1" [11, 8, 11] savanna_butchers_shop_1;
        "houses/savanna_butchers_shop_2" [13, 10, 9] savanna_butchers_shop_2;
        "houses/savanna_cartographer_1" [8, 8, 9] savanna_cartographer_1;
        "houses/savanna_fisher_cottage_1" [8, 11, 9] savanna_fisher_cottage_1;
        "houses/savanna_fletcher_house_1" [11, 7, 9] savanna_fletcher_house_1;
        "houses/savanna_large_farm_1" [9, 6, 9] savanna_large_farm_1;
        "houses/savanna_library_1" [10, 8, 8] savanna_library_1;
        "houses/savanna_mason_1" [8, 7, 10] savanna_mason_1;
        "houses/savanna_shepherd_1" [13, 14, 11] savanna_shepherd_1;
        "houses/savanna_small_farm" [6, 7, 9] savanna_small_farm;
        "houses/savanna_tannery_1" [8, 6, 9] savanna_tannery_1;
        "houses/savanna_temple_1" [13, 8, 9] savanna_temple_1;
        "houses/savanna_temple_2" [7, 7, 9] savanna_temple_2;
        "houses/savanna_tool_smith_1" [7, 7, 11] savanna_tool_smith_1;
        "houses/savanna_weaponsmith_1" [8, 6, 9] savanna_weaponsmith_1;
        "houses/savanna_weaponsmith_2" [9, 7, 13] savanna_weaponsmith_2;
        "savanna_lamp_post_01" [1, 2, 1] savanna_lamp_post_01;
        "town_centers/savanna_meeting_point_1" [14, 5, 12] |c| meeting_point_1(c, LIVING);
        "zombie/town_centers/savanna_meeting_point_1" [14, 6, 12] |c| meeting_point_1(c, ZOMBIE);
    }
}

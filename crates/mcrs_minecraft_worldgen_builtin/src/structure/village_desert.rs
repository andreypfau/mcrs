use super::*;
use mcrs_minecraft_core::{Axis, Direction};
use mcrs_minecraft_keys as keys;
use mcrs_minecraft_worldgen_structure::blueprint::{Cell, Hinge, Patch, top_stairs};

const SAND: &str = keys::block::SAND.as_static_str();
const SMOOTH: &str = keys::block::SMOOTH_SANDSTONE.as_static_str();
const CUT: &str = keys::block::CUT_SANDSTONE.as_static_str();
const SMOOTH_STAIRS: &str = keys::block::SMOOTH_SANDSTONE_STAIRS.as_static_str();
const SMOOTH_SLAB: &str = keys::block::SMOOTH_SANDSTONE_SLAB.as_static_str();
const SANDSTONE_STAIRS: &str = keys::block::SANDSTONE_STAIRS.as_static_str();
const JUNGLE_DOOR: &str = keys::block::JUNGLE_DOOR.as_static_str();
const CYAN_BED: &str = keys::block::CYAN_BED.as_static_str();
const GREEN_BED: &str = keys::block::GREEN_BED.as_static_str();
const LIME_BED: &str = keys::block::LIME_BED.as_static_str();

const HOUSE_LOOT: &str = "village_desert_house";

const STEP_SOUTH: &str = "minecraft:smooth_sandstone_stairs[facing=south]";

const CAMEL: &str =
    mcrs_minecraft_worldgen_feature::keys::template_pool::VILLAGE_DESERT_CAMEL.as_static_str();

kit! {
    K;
    smooth: block(SMOOTH),
    cut: block(CUT),
    terracotta: block(keys::block::TERRACOTTA.as_static_str()),
    sand: block(SAND),
    sandstone: block(keys::block::SANDSTONE.as_static_str()),
    slab: slab(SMOOTH_SLAB, "bottom"),
    slab_top: slab(SMOOTH_SLAB, "top"),
    slab_double: slab(SMOOTH_SLAB, "double"),
    stone_wall: settled("minecraft:sandstone_wall[waterlogged=false]"),
    potted_cactus: block(keys::block::POTTED_CACTUS.as_static_str()),
    potted_bush: block(keys::block::POTTED_DEAD_BUSH.as_static_str()),
    fence: settled("minecraft:jungle_fence[waterlogged=false]"),
}

fn sandstone_slab(kind: &str) -> Cell {
    slab(keys::block::SANDSTONE_SLAB.as_static_str(), kind)
}

fn button(facing: Direction) -> Cell {
    block(&format!(
        "minecraft:jungle_button[face=wall,facing={},powered=false]",
        facing.name()
    ))
}

/// A sandstone wall standing free of the block above it: a post joined low
/// to `sides`.
fn low_wall(sides: &[Direction]) -> Cell {
    wall_joined(keys::block::SANDSTONE_WALL.as_static_str(), sides, &[])
}

/// A smooth sandstone floor inside a ring of sand.
fn floor(c: &mut Canvas, min: [i32; 2], max: [i32; 2]) {
    c.solid(&K.sand, [min[0], 0, min[1]], [max[0], 0, max[1]]);
    c.solid(
        &K.smooth,
        [min[0] + 1, 0, min[1] + 1],
        [max[0] - 1, 0, max[1] - 1],
    );
}

fn glazed(colour: &str, facing: Direction) -> Cell {
    block(&format!(
        "minecraft:{colour}_glazed_terracotta[facing={}]",
        facing.name()
    ))
}

/// Four glazed tiles, each turned a quarter from the last round the corner
/// they share.
fn pinwheel(c: &mut Canvas, colour: &str, at: [i32; 3]) {
    let [x, y, z] = at;
    for (dx, dz, facing) in [(0, 0, North), (1, 0, East), (0, 1, West), (1, 1, South)] {
        c.place(&glazed(colour, facing), x + dx, y, z + dz);
    }
}

/// Niches in a wall: a potted cactus under a slab.
fn niches(c: &mut Canvas, lintel: &Cell, cells: &[[i32; 2]]) {
    for [x, z] in cells {
        c.place(&K.potted_cactus, *x, 2, *z);
        c.place(lintel, *x, 3, *z);
    }
}

fn hay(axis: Axis) -> Cell {
    log(keys::block::HAY_BLOCK.as_static_str(), axis)
}

/// A step under water, its shape left to the neighbours.
fn wet_step(facing: Direction) -> Cell {
    settled(&format!(
        "{SMOOTH_STAIRS}[facing={},half=bottom,waterlogged=true]",
        facing.name()
    ))
}

/// A step whose corner turns towards a neighbour that is not there.
fn corner_step(facing: Direction, shape: &str) -> Cell {
    corner_stairs(SMOOTH_STAIRS, facing, shape)
}

/// Windows in a wall: a sandstone slab on the sill and another under the
/// lintel.
fn slits(c: &mut Canvas, cells: &[[i32; 2]]) {
    for [x, z] in cells {
        c.place(&sandstone_slab("bottom"), *x, 2, *z);
        c.place(&sandstone_slab("top"), *x, 3, *z);
    }
}

/// A column of cut sandstone `height` tall under terracotta and a torch.
fn lamp(c: &mut Canvas, at: [i32; 3], height: i32) {
    let [x, y, z] = at;
    c.solid(&K.cut, [x, y, z], [x, y + height - 1, z]);
    c.place(&K.terracotta, x, y + height, z);
    c.place(&S.torch, x, y + height + 1, z);
}

/// Where the camel of a living village stands.
fn camel(c: &mut Canvas, at: [i32; 3], orientation: &str, floor: &str) {
    let jigsaw = block(&format!("minecraft:jigsaw[orientation={orientation}]"));
    c.place(&jigsaw, at[0], at[1], at[2]);
    c.jigsaw_aligned(at, EMPTY, CAMEL, floor);
}

/// A seat with a step against it on the north and on the south.
fn bench(c: &mut Canvas, at: [i32; 3], steps: &str, seat: &Cell) {
    let [x, y, z] = at;
    c.place(&stairs(steps, North), x, y, z - 1);
    c.place(seat, x, y, z);
    c.place(&stairs(steps, South), x, y, z + 1);
}

fn path(c: &mut Canvas, along: Axis, middle: i32, range: [i32; 2]) {
    path_strip(c, &K.smooth, along, middle, range);
}

/// The west end of a street that nothing joins.
fn dead_end(c: &mut Canvas, at: [i32; 3]) {
    c.socket(at, "minecraft:street", EMPTY, NOTHING);
}

fn corner_01(c: &mut Canvas, v: Village) {
    street(c, v, &[[5, 0], [0, 4]]);
    c.place(&S.air, 5, 1, 5);
    path(c, Z, 5, [0, 3]);
    c.solid(&K.smooth, [0, 0, 3], [5, 0, 5]);
    houses(c, v, South, 5, [3, 3]);
    decorations(c, v, SAND, &[[2, 1]]);
    c.spot([5, 0, 5], CATS, SAND);
}

fn corner_02(c: &mut Canvas, v: Village) {
    street(c, v, &[[0, 1], [4, 5]]);
    c.place(&S.air, 0, 1, 2);
    c.rows(&K.smooth, 0, 0, &[[0, 3], [0, 4], [0, 5]]);
    path(c, Z, 4, [3, 5]);
    decorations(c, v, SAND, &[[1, 4]]);
}

fn crossroad_01(c: &mut Canvas, v: Village) {
    street(c, v, &[[7, 0], [0, 7], [7, 14]]);
    c.solid(&S.air, [5, 1, 6], [6, 1, 6]);
    path(c, Z, 7, [0, 14]);
    path(c, X, 7, [0, 8]);
    houses(c, v, North, 6, [2, 3]);
    houses(c, v, South, 8, [2, 3]);
    houses(c, v, East, 8, [2, 3]);
    houses(c, v, East, 8, [7, 7]);
    decorations(c, v, SAND, &[[10, 12]]);
    c.spot([5, 0, 7], CATS, SMOOTH);
}

fn crossroad_02(c: &mut Canvas, v: Village) {
    street(c, v, &[[5, 0], [0, 5], [10, 5], [5, 10]]);
    path(c, Z, 5, [0, 10]);
    path(c, X, 5, [0, 10]);
    for (outer, inner) in [(2, 3), (8, 7)] {
        c.fill(&K.terracotta, [3, 7], 0, outer);
        c.fill(&K.terracotta, [2, 3, 7, 8], 0, inner);
    }
    decorations(c, v, SAND, &[[9, 1], [5, 5], [1, 8]]);
}

fn crossroad_03(c: &mut Canvas, v: Village) {
    c.jigsaw_layer(1);
    c.rows(&S.air, 1, 1, &[[3, 3], [2, 3], [0, 4], [0, 4]]);
    street_ends(c, v, &[[2, 0], [0, 2], [4, 2], [2, 4]]);
    path(c, Z, 2, [0, 4]);
    path(c, X, 2, [0, 4]);
}

fn square_01(c: &mut Canvas, v: Village) {
    street(c, v, &[[0, 12], [12, 12]]);
    let paving = [5, 10, 1, 11, 0, 12, 0, 12, 0, 12, 1, 11, 2, 10];
    c.rows(&K.smooth, 0, 9, paving.as_chunks().0);
    checker(c, [&K.smooth, &K.terracotta], 0, [4, 11], [8, 13]);
    houses(c, v, North, 9, [7, 10]);
    houses(c, v, North, 10, [2, 4]);
    houses(c, v, South, 15, [2, 10]);
    decorations(c, v, SMOOTH, &[[10, 10], [3, 13]]);
}

fn square_02(c: &mut Canvas, v: Village) {
    c.jigsaw_layer(1);
    dead_end(c, [0, 1, 8]);
    let paving = [[3, 6], [2, 7], [0, 7], [0, 7], [0, 7], [2, 7], [3, 6]];
    c.rows(&K.smooth, 0, 5, &paving);
    houses(c, v, North, 5, [4, 4]);
    houses(c, v, South, 11, [5, 5]);
    houses(c, v, East, 7, [8, 8]);
    decorations(c, v, SAND, &[[4, 8], [0, 10]]);
}

/// A street from west to east along the row `middle`, with places for houses
/// on its `side` at three points.
fn straight_with_houses(c: &mut Canvas, v: Village, middle: i32, side: Direction, at: [i32; 3]) {
    street(c, v, &[[0, middle], [14, middle]]);
    path(c, X, middle, [0, 14]);
    let line = if side == South {
        middle + 1
    } else {
        middle - 1
    };
    for x in at {
        houses(c, v, side, line, [x, x]);
    }
}

fn straight_01(c: &mut Canvas, v: Village) {
    straight_with_houses(c, v, 2, South, [2, 7, 12]);
    c.place(&S.air, 1, 1, 0);
    c.place(&block(keys::block::DEAD_BUSH.as_static_str()), 13, 0, 6);
    decorations(c, v, SAND, &[[1, 0], [13, 0]]);
}

fn straight_02(c: &mut Canvas, v: Village) {
    straight_with_houses(c, v, 14, North, [3, 7, 11]);
    decorations(c, v, SAND, &[[5, 16], [12, 17]]);
}

fn straight_03(c: &mut Canvas, v: Village) {
    street(c, v, &[[0, 1], [3, 1]]);
    path(c, X, 1, [0, 3]);
}

fn turn_01(c: &mut Canvas, v: Village) {
    street(c, v, &[[2, 0], [0, 2]]);
    c.rows(&K.smooth, 0, 0, &[[1, 3], [0, 3], [0, 2], [0, 1]]);
    decorations(c, v, SAND, &[[3, 3]]);
}

fn terminator_01(c: &mut Canvas) {
    c.jigsaw_layer(1);
    c.rows(&K.smooth, 0, 0, &[[0, 0], [0, 2], [0, 1]]);
    dead_end(c, [0, 1, 1]);
}

fn terminator_02(c: &mut Canvas, v: Village) {
    c.void([0, 1, 0], [2, 1, 0]);
    c.rows(&K.smooth, 0, 0, &[[0, 1], [0, 2], [0, 1]]);
    dead_end(c, [0, 1, 1]);
    decorations(c, v, SAND, &[[2, 2]]);
}

fn camel_spawn(c: &mut Canvas) {
    c.place(&block("minecraft:jigsaw[orientation=down_north]"), 0, 0, 0);
    c.jigsaw_aligned([0, 0, 0], EMPTY, EMPTY, AIR);
    c.entity([0.0, 0.0, 0.0], [0, 0, 1], &[CAMEL_ENTITY]);
}

fn desert_lamp_1(c: &mut Canvas) {
    c.socket_facing([0, 0, 0], "down_south", BOTTOM, EMPTY, CUT);
    lamp(c, [0, 1, 0], 1);
}

fn small_house_1(c: &mut Canvas, v: Village) {
    let eave = sandstone_slab("bottom");
    floor(c, [1, 0], [5, 4]);
    c.solid(&K.smooth, [0, 0, 2], [1, 0, 2]);
    c.walls(&K.smooth, &K.sandstone, [1, 1, 0], [5, 1, 4]);
    c.walls(&K.smooth, &K.smooth, [1, 2, 0], [5, 3, 4]);
    c.walls(&K.smooth, &eave, [1, 4, 0], [5, 4, 4]);
    c.patch(&eave, 5, &Patch::clipped_rectangle([1, 0], [5, 4], 1));
    c.place(&K.sandstone, 5, 1, 2);
    niches(c, &sandstone_slab("top"), &[[5, 2]]);
    c.door(JUNGLE_DOOR, [1, 1, 2], West, Hinge::Left);
    c.place(&sandstone_slab("double"), 1, 3, 2);
    c.entrance([0, 1, 2], EMPTY, NOTHING);
    c.bed(CYAN_BED, [3, 1, 3], East);
    c.place(&stairs(SMOOTH_STAIRS, East), 4, 1, 1);
    c.each(&S.wall_torch, &[[0, 3, 2], [2, 4, 2]]);
    villagers(c, v, SMOOTH, &[[3, 0, 2]]);
}

fn small_house_2(c: &mut Canvas, v: Village) {
    floor(c, [2, 0], [6, 4]);
    c.void([0, 0, 0], [1, 0, 4]);
    c.fill(&K.sand, 0, 0, [0, 4]);
    c.solid(&K.smooth, [0, 0, 2], [2, 0, 2]);
    c.walls(&K.smooth, &K.cut, [2, 1, 0], [6, 3, 4]);
    c.walls(&K.cut, &K.cut, [2, 4, 0], [6, 4, 4]);
    c.solid(&K.smooth, [2, 4, 1], [2, 4, 3]);
    c.solid(&K.slab, [2, 5, 1], [6, 5, 3]);
    c.fill(&K.slab_double, [2, 6], 5, 2);
    niches(c, &K.slab_top, &[[6, 2]]);

    c.fill(&K.stone_wall, 0, 1..=3, [0, 4]);
    c.solid(&K.slab, [0, 4, 0], [0, 4, 4]);
    c.solid(&K.slab_top, [1, 4, 1], [1, 4, 3]);
    c.fill(&K.slab_double, 1, 4, [0, 4]);
    c.door(JUNGLE_DOOR, [2, 1, 2], West, Hinge::Right);
    c.entrance([0, 1, 2], EMPTY, NOTHING);
    c.place(&S.wall_torch, 1, 3, 2);

    c.fill(
        &block(keys::block::GREEN_CARPET.as_static_str()),
        3,
        1,
        [1, 3],
    );
    c.place(
        &block(keys::block::CHISELED_SANDSTONE.as_static_str()),
        5,
        1,
        1,
    );
    c.place(&S.torch, 5, 2, 1);
    c.bed(GREEN_BED, [5, 1, 2], South);
    villagers(c, v, SAND, &[[1, 0, 1]]);
    c.spot([0, 0, 3], CATS, SAND);
}

fn small_house_3(c: &mut Canvas, v: Village) {
    floor(c, [0, 0], [4, 4]);
    c.solid(&K.sand, [0, 0, 5], [4, 0, 5]);
    c.solid(&K.smooth, [2, 0, 4], [2, 0, 5]);
    c.walls(&K.smooth, &K.cut, [0, 1, 0], [4, 3, 4]);
    c.solid(&K.slab_top, [1, 3, 1], [3, 3, 3]);
    c.scatter(&K.slab_top, 3, &[[2, 0], [0, 2], [4, 2]]);
    c.fill(&K.slab_double, [1, 3], 3, 4);
    for z in [0, 4] {
        c.fill(&stairs(SMOOTH_STAIRS, East), [0, 3], 4, z);
        c.fill(&stairs(SMOOTH_STAIRS, West), [1, 4], 4, z);
        c.place(&K.slab, 2, 4, z);
    }
    c.door(JUNGLE_DOOR, [2, 1, 4], South, Hinge::Left);
    c.entrance([2, 1, 5], EMPTY, NOTHING);
    c.solid(&button(South), [1, 3, 5], [3, 3, 5]);

    bench(c, [1, 1, 2], SMOOTH_STAIRS, &K.cut);
    c.place(&K.potted_bush, 1, 2, 2);
    c.bed(GREEN_BED, [3, 1, 2], North);
    c.fill(&wall_torch(North), [1, 3], 2, 3);
    villagers(c, v, SMOOTH, &[[2, 0, 1]]);
}

fn small_house_4(c: &mut Canvas, v: Village) {
    floor(c, [0, 0], [4, 4]);
    c.place(&K.terracotta, 2, 0, 2);
    c.place(&K.smooth, 2, 0, 4);
    c.walls(&K.smooth, &K.cut, [0, 1, 0], [4, 3, 4]);
    c.place(&S.air, 2, 2, 0);
    c.place(&K.slab_top, 2, 3, 0);

    c.fill(&K.terracotta, [0, 2, 4], 4, [0, 2, 4]);
    c.solid(&K.slab_top, [1, 4, 1], [3, 4, 3]);
    c.fill(&stairs(SMOOTH_STAIRS, South), [1, 3], 4, 0);
    c.fill(&stairs(SMOOTH_STAIRS, North), [1, 3], 4, 4);
    c.fill(&stairs(SMOOTH_STAIRS, East), 0, 4, [1, 3]);
    c.fill(&stairs(SMOOTH_STAIRS, West), 4, 4, [1, 3]);

    c.door(JUNGLE_DOOR, [2, 1, 4], South, Hinge::Left);
    c.entrance([3, 1, 4], EMPTY, SMOOTH);
    c.bed(CYAN_BED, [2, 1, 1], West);
    chest(c, [3, 1, 1], South, HOUSE_LOOT);
    c.place(&S.wall_torch, 2, 3, 3);
    villagers(c, v, SMOOTH, &[[3, 0, 2]]);
}

fn small_house_5(c: &mut Canvas, v: Village) {
    c.solid(&K.sand, [0, 0, 0], [4, 0, 5]);
    c.solid(&K.smooth, [2, 0, 0], [2, 0, 1]);
    checker(c, [&K.smooth, &K.terracotta], 0, [1, 2], [3, 4]);
    c.walls(&K.smooth, &K.smooth, [0, 1, 1], [4, 3, 5]);
    c.fill(&K.cut, [0, 4], 1..=3, [2, 4]);
    c.fill(&K.cut, [1, 3], 1..=2, 5);
    c.fill(&S.air, [1, 3], 3, 5);
    c.solid(&K.cut, [0, 4, 1], [4, 4, 5]);
    c.fill(&K.cut, [0, 2, 4], 5, [1, 5]);
    c.fill(&K.cut, [0, 4], 5, 3);

    c.door(JUNGLE_DOOR, [2, 1, 1], North, Hinge::Left);
    c.entrance([2, 1, 0], EMPTY, NOTHING);
    c.place(&K.potted_cactus, 1, 1, 0);
    c.solid(&button(North), [1, 3, 0], [3, 3, 0]);
    c.place(&S.crafting_table, 1, 1, 2);
    c.bed(LIME_BED, [1, 1, 3], South);
    c.place(&S.wall_torch, 2, 3, 4);
    villagers(c, v, SMOOTH, &[[3, 0, 3]]);
}

/// The watchtower: a spiral stair of a step and a landing a level, a slit
/// over the door on every turn. The abandoned one has lost its porch and
/// the torches on its battlements, and is entered through its wall.
fn small_house_6(c: &mut Canvas, v: Village) {
    let west = if v.zombie { 0 } else { 1 };
    let x = |from_wall: i32| west + from_wall;
    c.solid(&K.sand, [0, 0, 0], [x(4), 0, 4]);
    c.solid(&K.smooth, [x(1), 0, 1], [x(2), 0, 1]);
    c.solid(&K.smooth, [0, 0, 2], [x(2), 0, 2]);
    c.place(&K.smooth, x(2), 0, 3);

    c.walls(&K.smooth, &K.cut, [x(0), 1, 0], [x(4), 14, 4]);
    for turn in 0..4 {
        let y = 1 + 4 * turn;
        c.place(&stairs(SMOOTH_STAIRS, East), x(2), y, 1);
        c.place(&K.smooth, x(3), y, 1);
        c.place(&stairs(SMOOTH_STAIRS, South), x(3), y + 1, 2);
        if turn < 3 {
            c.place(&K.smooth, x(3), y + 1, 3);
            c.place(&stairs(SMOOTH_STAIRS, West), x(2), y + 2, 3);
            c.place(&K.smooth, x(1), y + 2, 3);
            c.place(&stairs(SMOOTH_STAIRS, North), x(1), y + 3, 2);
            c.place(&K.smooth, x(1), y + 3, 1);
            c.place(&K.cut, x(0), y + 2, 2);
            c.place(&S.air, x(0), y + 4, 2);
        }
    }
    c.solid(&K.smooth, [x(3), 1, 2], [x(3), 1, 3]);
    c.each(&K.smooth, &[[x(2), 5, 2], [x(2), 12, 2]]);

    c.solid(&K.cut, [x(0), 15, 0], [x(4), 15, 4]);
    c.solid(&S.air, [x(1), 15, 1], [x(3), 15, 1]);
    c.place(&S.air, x(3), 15, 2);
    c.place(&stairs(SMOOTH_STAIRS, South), x(3), 15, 3);
    c.fill(&K.cut, [x(0), x(2), x(4)], 16, [0, 4]);
    c.fill(&K.cut, [x(0), x(4)], 16, 2);

    c.door(JUNGLE_DOOR, [x(0), 1, 2], West, Hinge::Right);
    c.bed(GREEN_BED, [x(2), 1, 2], South);
    chest(c, [x(2), 13, 2], West, HOUSE_LOOT);
    let torches = [
        (East, [1, 2, 1]),
        (West, [3, 4, 3]),
        (South, [3, 7, 1]),
        (North, [1, 9, 3]),
        (North, [3, 12, 3]),
    ];
    for (facing, [from_wall, y, z]) in torches {
        c.place(&wall_torch(facing), x(from_wall), y, z);
    }
    villagers(c, v, SMOOTH, &[[x(1), 0, 3]]);
    if v.zombie {
        c.entrance([0, 1, 3], EMPTY, SMOOTH);
    } else {
        c.entrance([0, 1, 2], EMPTY, AIR);
        c.place(&S.wall_torch, 0, 3, 2);
        c.fill(&S.torch, [1, 5], 17, [0, 4]);
    }
}

fn small_house_7(c: &mut Canvas, v: Village) {
    c.solid(&K.sand, [0, 0, 0], [7, 0, 6]);
    c.solid(&K.smooth, [0, 0, 1], [6, 0, 1]);
    c.solid(&K.smooth, [5, 0, 2], [6, 0, 5]);
    c.place(&K.smooth, 4, 0, 4);
    c.void([1, 0, 3], [3, 0, 5]);
    c.each(&S.air, &[[1, 0, 3], [3, 0, 5]]);
    c.place(&K.sand, 2, 0, 4);

    c.walls(&K.smooth, &K.smooth, [4, 1, 0], [7, 3, 6]);
    c.solid(&K.slab_top, [5, 3, 1], [6, 3, 5]);
    c.fill(&K.slab, [5, 6], 2, [0, 6]);
    c.fill(&K.slab, 7, 2, [2, 4]);
    c.fill(&K.slab, [4, 7], 4, [0, 2, 4, 6]);
    c.door(JUNGLE_DOOR, [4, 1, 1], West, Hinge::Left);
    c.door(JUNGLE_DOOR, [4, 1, 4], West, Hinge::Right);
    c.entrance([0, 1, 1], EMPTY, NOTHING);
    bench(c, [6, 1, 2], SMOOTH_STAIRS, &K.cut);
    c.place(&S.torch, 6, 2, 2);
    c.bed(LIME_BED, [6, 1, 4], South);

    c.fill(&K.stone_wall, 0..=3, 1, [0, 2, 6]);
    c.solid(&K.stone_wall, [0, 1, 3], [0, 1, 5]);
    c.place(&low_wall(&[East]), 0, 1, 0);
    c.place(&low_wall(&[East, South]), 0, 1, 2);
    c.place(&low_wall(&[North, South]), 0, 1, 4);
    c.place(&low_wall(&[East, West]), 2, 1, 6);
    c.fill(&S.torch, 2, 2, [0, 2]);
    c.each(&K.potted_bush, &[[0, 2, 4], [2, 2, 6]]);
    c.solid(&block("minecraft:cactus[age=0]"), [2, 1, 4], [2, 3, 4]);
    villagers(c, v, SMOOTH, &[[5, 0, 3]]);
}

fn small_house_8(c: &mut Canvas, v: Village) {
    floor(c, [0, 0], [4, 4]);
    c.place(&K.smooth, 2, 0, 0);
    c.walls(&K.smooth, &K.terracotta, [0, 1, 0], [4, 3, 4]);
    c.walls(
        &sandstone_slab("bottom"),
        &K.terracotta,
        [0, 4, 0],
        [4, 4, 4],
    );
    c.solid(&K.slab, [1, 4, 1], [3, 4, 3]);
    c.each(&K.slab_double, &[[4, 2, 2], [2, 2, 4]]);
    c.scatter(&K.slab_top, 3, &[[0, 2], [4, 2], [2, 4]]);
    c.door(JUNGLE_DOOR, [2, 1, 0], North, Hinge::Right);
    c.entrance([1, 1, 0], EMPTY, SMOOTH);

    bench(c, [1, 1, 2], SANDSTONE_STAIRS, &K.terracotta);
    c.place(
        &block("minecraft:sea_pickle[pickles=2,waterlogged=false]"),
        1,
        2,
        2,
    );
    c.bed(LIME_BED, [3, 1, 2], North);
    c.place(&S.crafting_table, 3, 1, 3);
    c.place(&S.wall_torch, 2, 3, 1);
    villagers(c, v, SMOOTH, &[[2, 0, 2]]);
}

fn medium_house_1(c: &mut Canvas, v: Village) {
    let sill = sandstone_slab("bottom");
    c.solid(&K.sand, [0, 0, 0], [0, 0, 6]);
    floor(c, [1, 0], [5, 6]);
    c.place(&K.smooth, 1, 0, 3);
    c.walls(&K.smooth, &K.smooth, [1, 1, 0], [5, 3, 6]);
    slits(c, &[[3, 0], [1, 1], [5, 2], [5, 4], [1, 5], [3, 6]]);
    c.solid(&K.cut, [1, 4, 0], [5, 4, 6]);
    c.fill(&sill, [1, 3, 5], 5, [0, 6]);
    c.fill(&sill, [1, 5], 5, 3);

    c.door(JUNGLE_DOOR, [1, 1, 3], West, Hinge::Right);
    c.place(&K.potted_cactus, 0, 1, 1);
    c.fill(&S.wall_torch, 0, 2, [2, 4]);
    chest(c, [2, 1, 1], South, HOUSE_LOOT);
    for z in [1, 5] {
        c.bed(LIME_BED, [3, 1, z], East);
        c.place(&wall_torch(West), 4, 3, z);
    }
    c.place(&K.terracotta, 4, 1, 3);
    c.place(&K.potted_cactus, 4, 2, 3);
    c.place(&S.wall_torch, 2, 3, 3);
    if v.zombie {
        c.entrance([0, 1, 3], EMPTY, NOTHING);
        villagers(c, v, NOTHING, &[[3, 0, 2]]);
    } else {
        c.entrance([0, 1, 3], EMPTY, AIR);
        villagers(c, v, SMOOTH, &[[3, 0, 2], [3, 0, 4]]);
    }
}

fn medium_house_2(c: &mut Canvas, v: Village) {
    let ladder = block("minecraft:ladder[facing=west,waterlogged=false]");
    c.solid(&K.sand, [0, 0, 0], [10, 0, 6]);
    c.solid(&K.smooth, [2, 0, 2], [4, 0, 4]);
    c.solid(&K.smooth, [4, 0, 0], [4, 0, 1]);

    c.walls(&K.smooth, &K.smooth, [1, 1, 1], [5, 2, 5]);
    c.solid(&K.smooth, [1, 3, 1], [5, 3, 5]);
    c.walls(&K.smooth, &K.smooth, [1, 4, 3], [5, 6, 5]);
    c.solid(&K.slab, [1, 7, 3], [5, 7, 5]);
    c.fill(&K.slab_double, [1, 5], 7, [3, 5]);
    c.solid(&K.slab, [1, 4, 1], [4, 4, 1]);
    c.place(&K.slab, 1, 4, 2);
    c.place(&K.potted_cactus, 4, 4, 2);

    c.solid(&K.smooth, [5, 1, 1], [9, 2, 4]);
    c.walls(&K.smooth, &K.smooth, [5, 3, 1], [9, 5, 4]);
    c.solid(&K.slab_top, [6, 5, 2], [8, 5, 3]);
    c.scatter(&K.slab, 6, &[[5, 1], [9, 1], [6, 4], [9, 4]]);
    for step in 0..2 {
        let y = 1 + step;
        c.place(&stairs(SMOOTH_STAIRS, East), 5 + step, y, 0);
        c.solid(&K.smooth, [6 + step, y, 0], [8 - step, y, 0]);
        c.place(&stairs(SMOOTH_STAIRS, West), 9 - step, y, 0);
    }

    c.fill(&button(North), [1..=6, 8..=9], 3, 0);
    c.solid(&button(West), [0, 3, 1], [0, 3, 5]);
    c.solid(&button(East), [10, 3, 1], [10, 3, 4]);
    c.solid(&button(South), [6, 3, 5], [9, 3, 5]);
    c.solid(&button(North), [1, 6, 2], [5, 6, 2]);
    c.solid(&button(West), [0, 6, 3], [0, 6, 5]);
    c.fill(&button(East), 6, 6, [3, 5]);
    c.fill(&button(South), 1..=5, [3, 6], 6);

    for at in [[4, 1, 1], [7, 3, 1], [3, 4, 3]] {
        c.door(JUNGLE_DOOR, at, North, Hinge::Right);
    }
    c.entrance([4, 1, 0], EMPTY, NOTHING);
    c.place(&K.potted_cactus, 3, 1, 0);
    c.place(&S.wall_torch, 7, 5, 0);

    c.bed(GREEN_BED, [2, 1, 3], South);
    c.place(&S.wall_torch, 2, 2, 3);
    c.solid(&ladder, [4, 2, 4], [4, 3, 4]);
    c.bed(LIME_BED, [7, 3, 3], West);
    chest(c, [8, 3, 3], North, HOUSE_LOOT);
    c.fill(&wall_torch(South), [6, 8], 4, 2);
    c.place(&wall_torch(East), 2, 5, 4);
    villagers(c, v, SMOOTH, &[[4, 0, 2], [7, 2, 2]]);
}

fn desert_armorer_1(c: &mut Canvas) {
    let granite = block(keys::block::GRANITE.as_static_str());
    let granite_wall = settled("minecraft:granite_wall[waterlogged=false]");
    let button = block("minecraft:stone_button[face=wall,facing=west,powered=false]");
    c.void([0, 0, 0], [0, 0, 0]);
    c.solid(&K.sand, [0, 0, 1], [6, 0, 5]);
    c.fill(&K.sand, 2..=6, 0, [0, 6]);
    c.rows(&K.smooth, 0, 1, &[[3, 5], [1, 4], [0, 4], [1, 4], [3, 5]]);

    c.walls(&K.smooth, &K.cut, [2, 1, 0], [6, 4, 6]);
    c.solid(&S.air, [2, 1, 2], [2, 3, 4]);
    c.solid(&K.slab, [2, 5, 0], [6, 5, 6]);
    c.fill(&K.cut, [2, 6], 5, [0, 6]);
    c.fill(&K.slab_double, 4, 5, [0, 6]);
    c.fill(&K.slab_double, [2, 6], 5, [2, 4]);

    c.solid(&K.smooth, [0, 1, 2], [0, 3, 4]);
    c.solid(&K.slab, [0, 4, 2], [1, 4, 4]);
    c.place(&K.slab_double, 0, 4, 3);
    for z in [1, 5] {
        c.solid(&K.cut, [0, 1, z], [0, 4, z]);
        c.solid(&K.smooth, [1, 1, z], [1, 3, z]);
        c.place(&K.fence, 1, 2, z);
        c.place(&K.slab, 1, 4, z);
    }
    c.door(JUNGLE_DOOR, [0, 1, 3], West, Hinge::Right);
    c.entrance([0, 1, 4], EMPTY, SMOOTH);
    c.place(&S.wall_torch, 1, 3, 3);

    c.place(&stairs(SMOOTH_STAIRS, North), 3, 1, 1);
    c.place(&stairs(SMOOTH_STAIRS, South), 3, 1, 5);
    c.solid(&granite, [5, 1, 2], [5, 2, 4]);
    c.solid(&granite, [5, 3, 3], [5, 4, 3]);
    c.furnace([5, 1, 3], "blast_furnace", West);
    c.solid(&button, [4, 2, 2], [4, 2, 4]);
    c.place(
        &stairs(keys::block::GRANITE_STAIRS.as_static_str(), South),
        5,
        3,
        2,
    );
    c.place(
        &stairs(keys::block::GRANITE_STAIRS.as_static_str(), North),
        5,
        3,
        4,
    );
    c.solid(&granite_wall, [5, 5, 3], [5, 6, 3]);
    c.place(&wall_torch(North), 5, 2, 1);
    c.place(&wall_torch(South), 5, 2, 5);
}

fn desert_butcher_shop_1(c: &mut Canvas) {
    let counter = block("minecraft:smooth_stone_slab[type=double,waterlogged=false]");
    c.solid(&K.cut, [1, 0, 0], [7, 0, 7]);
    c.boxes(&S.grass, &[([5, 0, 1], [6, 0, 6]), ([2, 0, 5], [6, 0, 6])]);
    c.solid(&K.smooth, [2, 0, 1], [4, 0, 3]);
    c.place(&K.smooth, 1, 0, 2);
    c.solid(&counter, [2, 0, 2], [3, 0, 2]);
    c.place(&stairs(SMOOTH_STAIRS, East), 0, 0, 2);
    c.entrance([0, 0, 3], EMPTY, NOTHING);

    c.walls(&K.smooth, &K.cut, [1, 1, 0], [4, 3, 4]);
    c.fill(&K.slab, 2..=3, 2, [0, 4]);
    c.walls(&K.cut, &K.slab, [1, 4, 0], [4, 4, 4]);
    c.solid(&K.slab, [2, 4, 1], [3, 4, 3]);
    c.door(JUNGLE_DOOR, [1, 1, 2], West, Hinge::Right);
    c.door(JUNGLE_DOOR, [4, 1, 2], East, Hinge::Right);
    c.furnace([3, 1, 1], "smoker", South);
    c.solid(&K.terracotta, [3, 2, 1], [3, 4, 1]);
    c.solid(&counter, [2, 1, 3], [3, 1, 3]);
    c.place(&S.wall_torch, 0, 3, 2);
    c.place(&wall_torch(West), 3, 3, 2);

    c.solid(&K.stone_wall, [5, 1, 0], [7, 1, 0]);
    c.solid(&K.stone_wall, [7, 1, 1], [7, 1, 7]);
    c.solid(&K.stone_wall, [1, 1, 7], [6, 1, 7]);
    c.solid(&K.stone_wall, [1, 1, 5], [1, 1, 6]);
    c.each(&S.torch, &[[7, 2, 0], [1, 2, 7], [7, 2, 7]]);
    c.spot([6, 0, 6], BUTCHER_ANIMALS, GRASS);
}

fn desert_cartographer_house_1(c: &mut Canvas) {
    c.solid(&K.smooth, [0, 0, 1], [5, 1, 2]);
    c.solid(&S.air, [1, 0, 1], [3, 0, 1]);
    c.place(&K.smooth, 0, 0, 3);
    c.place(&stairs(SMOOTH_STAIRS, North), 0, 0, 4);
    c.place(&stairs(SMOOTH_STAIRS, North), 0, 1, 3);
    c.entrance([0, 0, 5], EMPTY, NOTHING);
    c.place(&K.potted_cactus, 0, 0, 0);
    c.place(&stairs(SMOOTH_STAIRS, West), 6, 0, 0);
    c.solid(&K.slab, [6, 0, 1], [6, 0, 2]);
    c.place(&stairs(SMOOTH_STAIRS, South), 5, 1, 1);
    c.place(&stairs(SMOOTH_STAIRS, South), 5, 2, 2);
    c.spot([5, 0, 0], CATS, SMOOTH);

    c.solid(&K.smooth, [2, 0, 0], [3, 4, 0]);
    c.solid(&K.smooth, [4, 2, 1], [4, 4, 1]);
    c.place(&K.smooth, 1, 4, 1);
    c.place(&K.smooth, 4, 2, 2);
    c.posts(&K.cut, &[1, 4], &[0], [0, 5]);
    c.solid(&K.cut, [1, 2, 2], [1, 5, 2]);
    c.solid(&K.cut, [4, 3, 2], [4, 5, 2]);
    c.solid(&K.slab, [1, 5, 1], [4, 5, 1]);
    c.fill(&K.slab, 2..=3, 5, [0, 2]);
    c.door(JUNGLE_DOOR, [1, 2, 1], West, Hinge::Right);
    c.place(
        &block(keys::block::CARTOGRAPHY_TABLE.as_static_str()),
        3,
        2,
        2,
    );
    c.place(&wall_torch(North), 3, 3, 2);

    c.solid(&K.smooth, [1, 0, 3], [6, 2, 6]);
    c.solid(&S.air, [3, 0, 4], [5, 1, 5]);
    c.solid(&S.air, [3, 0, 3], [3, 1, 3]);
    c.place(&S.air, 2, 0, 4);
    c.place(&wall_torch(South), 2, 1, 4);
    c.walls(&K.smooth, &K.cut, [3, 3, 3], [6, 5, 6]);
    c.solid(&S.air, [4, 3, 3], [5, 4, 3]);
    c.solid(&K.slab, [3, 6, 3], [6, 6, 6]);
    c.fill(&K.cut, [3, 6], 6, [3, 6]);
    c.solid(&K.cut, [6, 0, 3], [6, 2, 3]);
    c.fill(&K.cut, [1, 3, 6], 0..=2, 6);
    c.place(&K.cut, 1, 3, 6);
    c.place(&stairs(SMOOTH_STAIRS, South), 2, 2, 3);
    c.solid(&K.slab, [1, 3, 3], [1, 3, 5]);
    c.place(&K.slab, 2, 3, 6);
    c.place(&K.potted_cactus, 2, 3, 5);
    c.door(JUNGLE_DOOR, [3, 3, 4], West, Hinge::Left);
    c.place(&wall_torch(South), 5, 5, 4);
}

fn desert_library_1(c: &mut Canvas) {
    c.solid(&K.sand, [0, 0, 0], [8, 0, 4]);
    c.solid(&K.smooth, [2, 0, 1], [6, 0, 3]);
    c.place(&K.smooth, 4, 0, 0);
    c.walls(&K.smooth, &K.cut, [0, 1, 0], [8, 3, 4]);
    c.solid(&K.smooth, [0, 4, 0], [8, 4, 4]);
    c.walls(&K.cut, &K.cut, [0, 5, 0], [8, 5, 4]);
    c.posts(&K.cut, &[0, 8], &[0, 4], [4, 6]);
    niches(c, &sandstone_slab("top"), &[[2, 0], [6, 0], [2, 4], [6, 4]]);
    c.door(JUNGLE_DOOR, [4, 1, 0], North, Hinge::Left);
    c.place(&sandstone_slab("double"), 4, 3, 0);
    c.entrance([3, 1, 0], EMPTY, SMOOTH);

    c.fill(&stairs(SMOOTH_STAIRS, West), 1, 1, [1, 3]);
    c.lectern([1, 1, 2], East);
    c.fill(
        &block(keys::block::WHITE_CARPET.as_static_str()),
        [3, 5],
        1,
        2,
    );
    c.place(&block(keys::block::LIME_CARPET.as_static_str()), 4, 1, 2);
    c.solid(
        &block(keys::block::BOOKSHELF.as_static_str()),
        [7, 1, 1],
        [7, 3, 3],
    );
    c.each(&S.wall_torch, &[[1, 2, 2], [4, 2, 3]]);
    c.spot([2, 0, 3], CATS, SMOOTH);
}

fn desert_mason_1(c: &mut Canvas) {
    let lime = block(keys::block::LIME_TERRACOTTA.as_static_str());
    floor(c, [2, 0], [6, 7]);
    c.void([0, 0, 0], [1, 0, 7]);
    c.place(&S.air, 1, 0, 1);
    c.fill(&K.sand, 0, 0, [2, 5]);
    c.solid(&K.smooth, [0, 0, 3], [2, 0, 4]);

    c.walls(&K.smooth, &K.cut, [2, 1, 0], [6, 3, 7]);
    c.fill(&K.cut, 2, 1..=3, [2, 5]);
    c.solid(&K.cut, [2, 3, 3], [2, 3, 4]);
    c.fill(&K.slab, 4, 2, [0, 7]);
    c.solid(&K.slab, [6, 2, 2], [6, 2, 5]);
    c.solid(&K.slab, [2, 4, 0], [6, 4, 7]);
    c.solid(&K.slab, [0, 4, 2], [1, 4, 5]);
    c.fill(&K.cut, [2, 6], 4, [0, 7]);
    c.fill(&low_wall(&[]), 0, 1..=3, [2, 5]);
    c.fill(&S.wall_torch, 1, 2, [2, 5]);
    c.door(JUNGLE_DOOR, [2, 1, 3], West, Hinge::Right);
    c.door(JUNGLE_DOOR, [2, 1, 4], West, Hinge::Left);
    c.entrance([0, 1, 4], EMPTY, NOTHING);

    c.solid(&lime, [3, 1, 1], [4, 1, 1]);
    c.place(&lime, 3, 2, 1);
    c.place(&block(keys::block::CLAY.as_static_str()), 5, 1, 1);
    c.place(&glazed("white", South), 3, 1, 2);
    c.place(&block("minecraft:stonecutter[facing=north]"), 4, 1, 6);
    c.each(&S.wall_torch, &[[4, 3, 1], [4, 3, 6]]);
}

fn desert_shepherd_house_1(c: &mut Canvas) {
    let awning = sandstone_slab("bottom");
    c.solid(&K.sand, [0, 0, 0], [10, 0, 4]);
    c.place(&S.water, 1, 0, 1);
    c.solid(&K.smooth, [6, 0, 1], [8, 0, 1]);
    c.solid(&K.smooth, [4, 0, 2], [9, 0, 2]);
    c.place(&K.smooth, 6, 0, 3);

    c.solid(&awning, [0, 4, 0], [3, 4, 3]);
    c.posts(&K.cut, &[0], &[0, 3], [1, 4]);
    c.fill(&K.stone_wall, 1..=3, 1, [0, 3]);
    c.solid(&K.stone_wall, [0, 1, 1], [0, 1, 2]);
    c.void([1, 1, 1], [3, 1, 1]);
    c.void([1, 2, 0], [3, 2, 0]);
    c.void([0, 2, 1], [3, 2, 1]);
    c.void([1, 3, 0], [2, 3, 0]);
    c.void([0, 3, 1], [3, 3, 1]);
    c.place(&S.air, 1, 3, 1);
    c.void([0, 5, 0], [3, 5, 1]);
    c.fill(&S.wall_torch, 3, 3, [0, 3]);
    c.spot([3, 0, 2], SHEEP, SAND);

    c.walls(&K.smooth, &K.cut, [4, 1, 0], [10, 4, 3]);
    c.solid(&S.air, [4, 1, 1], [4, 2, 2]);
    slits(c, &[[6, 0], [8, 0], [8, 3]]);
    c.solid(&awning, [4, 5, 0], [10, 5, 3]);
    c.fill(&K.cut, [4, 10], 5, [0, 3]);
    c.door(JUNGLE_DOOR, [6, 1, 3], South, Hinge::Right);
    c.entrance([5, 1, 4], EMPTY, NOTHING);
    c.fill(&K.cut, [4, 8], 1, 4);
    c.fill(&K.potted_cactus, [4, 8], 2, 4);
    c.place(&S.wall_torch, 6, 3, 4);

    c.place(&hay(Y), 4, 1, 1);
    c.place(&hay(X), 5, 1, 1);
    c.solid(&block("minecraft:loom[facing=east]"), [9, 1, 1], [9, 1, 2]);
    c.place(&wall_torch(East), 5, 3, 1);
    c.solid(&wall_torch(West), [9, 3, 1], [9, 3, 2]);
}

fn desert_weaponsmith_1(c: &mut Canvas) {
    let cobble = block(keys::block::COBBLESTONE.as_static_str());
    let bars = settled("minecraft:iron_bars[waterlogged=false]");
    let end_bars = block(
        "minecraft:iron_bars[east=false,north=false,south=true,waterlogged=false,west=false]",
    );
    c.solid(&K.cut, [0, 0, 0], [4, 0, 5]);
    c.solid(&K.cut, [4, 0, 1], [9, 0, 6]);
    c.solid(&K.smooth, [1, 0, 1], [3, 0, 4]);
    c.entrance([5, 0, 0], EMPTY, STEP_SOUTH);
    c.solid(&stairs(SMOOTH_STAIRS, South), [6, 0, 0], [7, 0, 0]);

    c.walls(&K.smooth, &K.cut, [0, 1, 0], [4, 4, 5]);
    c.solid(&S.air, [4, 1, 1], [4, 2, 2]);
    c.place(&top_stairs(SMOOTH_STAIRS, North), 4, 3, 1);
    c.place(&top_stairs(SMOOTH_STAIRS, South), 4, 3, 2);
    c.solid(&K.cut, [4, 1, 3], [4, 4, 3]);
    c.place(&K.smooth, 4, 4, 5);
    slits(c, &[[2, 0], [2, 5]]);
    c.solid(&K.slab, [0, 5, 0], [4, 5, 5]);
    c.solid(&K.slab, [5, 5, 1], [8, 5, 6]);
    c.solid(&S.air, [6, 5, 3], [7, 5, 4]);
    c.fill(&K.cut, [0, 4], 5, [0, 5]);
    c.place(&K.slab, 4, 5, 5);

    c.solid(&cobble, [5, 1, 3], [8, 1, 5]);
    c.solid(&block("minecraft:lava[level=0]"), [6, 1, 4], [7, 1, 4]);
    c.solid(&cobble, [5, 2, 5], [8, 4, 5]);
    c.place(&cobble, 8, 1, 2);
    c.solid(&K.cut, [8, 1, 1], [9, 1, 1]);
    c.solid(&K.cut, [9, 1, 2], [9, 1, 4]);
    c.solid(&K.cut, [9, 1, 5], [9, 5, 5]);
    c.solid(&K.smooth, [5, 1, 6], [8, 4, 6]);
    c.posts(&K.cut, &[4, 9], &[6], [1, 5]);
    for y in [2, 3] {
        c.furnace([5, y, 4], "furnace", North);
    }
    c.solid(&low_wall(&[]), [8, 2, 1], [8, 4, 1]);
    c.solid(&end_bars, [8, 2, 2], [8, 4, 2]);
    c.solid(&bars, [8, 2, 3], [8, 4, 4]);
    c.place(&K.potted_cactus, 9, 2, 2);

    c.place(&stairs(SMOOTH_STAIRS, West), 1, 1, 1);
    chest(c, [1, 1, 2], East, "village_weaponsmith");
    c.place(
        &block("minecraft:grindstone[face=floor,facing=south]"),
        2,
        1,
        4,
    );
    c.place(&S.wall_torch, 3, 3, 3);
}

fn desert_fisher_1(c: &mut Canvas) {
    c.solid(&K.sand, [0, 0, 0], [7, 0, 9]);
    c.void([1, 0, 10], [7, 0, 10]);
    c.place(&K.sand, 0, 0, 10);
    c.solid(&K.cut, [1, 0, 2], [3, 0, 3]);
    c.rows(&K.smooth, 0, 4, &[[5, 6], [4, 6], [4, 6], [0, 6], [4, 6]]);
    c.void([1, 0, 5], [1, 0, 6]);
    c.void([0, 0, 8], [0, 0, 8]);
    c.void([1, 0, 9], [1, 0, 9]);
    c.spot([0, 0, 9], CATS, SAND);

    c.walls(&K.cut, &K.cut, [0, 1, 0], [4, 1, 4]);
    c.solid(&S.water, [1, 1, 1], [3, 1, 3]);
    c.posts(&K.cut, &[4], &[0, 2, 4], [2, 5]);
    c.posts(&K.cut, &[0, 2], &[4], [2, 5]);
    c.fill(&K.slab_top, 4, 5, [1, 3]);
    c.fill(&K.slab_top, [1, 3], 5, 4);
    c.place(&K.potted_bush, 3, 2, 0);
    c.place(&K.potted_cactus, 0, 2, 2);

    c.walls(&K.smooth, &K.cut, [3, 1, 5], [7, 3, 9]);
    c.solid(&S.air, [5, 1, 5], [6, 3, 5]);
    c.solid(&K.smooth, [7, 1, 5], [7, 2, 5]);
    c.solid(&K.smooth, [5, 1, 3], [7, 3, 4]);
    c.solid(&S.air, [6, 1, 4], [6, 3, 4]);
    c.posts(&K.cut, &[5, 7], &[3], [1, 3]);
    c.each(&K.slab_top, &[[7, 3, 5], [7, 3, 7], [5, 3, 9]]);
    c.solid(&K.slab, [3, 4, 5], [7, 4, 9]);
    c.solid(&K.slab, [5, 4, 3], [7, 4, 4]);
    c.fill(&K.cut, 3, 4, [5, 9]);
    c.place(&K.cut, 7, 4, 9);
    c.fill(&K.cut, [5, 7], 4, 3);
    c.door(JUNGLE_DOOR, [3, 1, 7], West, Hinge::Right);
    c.entrance([0, 1, 6], EMPTY, NOTHING);
    c.place(&S.wall_torch, 2, 3, 7);

    c.barrel([6, 1, 4], Direction::Up);
    c.place(&wall_torch(South), 6, 2, 4);
    c.barrel([5, 1, 8], East);
    c.barrel([6, 1, 8], West);
    c.barrel([6, 2, 8], North);
    c.place(&wall_torch(North), 5, 2, 8);
    c.barrel([2, 1, 5], South);
    c.barrel([2, 2, 5], Direction::Up);
    c.place(&K.potted_bush, 0, 1, 5);
    c.place(&K.potted_cactus, 2, 1, 6);
    c.place(&hay(X), 1, 1, 8);
    c.place(&hay(Y), 2, 1, 9);
    c.each(&hay(Z), &[[2, 2, 9], [0, 1, 10]]);
}

fn desert_tannery_1(c: &mut Canvas) {
    c.solid(&K.sand, [0, 0, 0], [6, 1, 5]);
    c.solid(&S.air, [6, 1, 1], [6, 1, 4]);
    c.solid(&K.smooth, [0, 1, 0], [6, 1, 0]);
    c.solid(&K.smooth, [1, 1, 1], [5, 1, 3]);
    c.fill(&S.air, [2, 4], 1, 2);
    c.solid(&K.smooth, [1, 1, 4], [3, 1, 4]);

    c.walls(&K.smooth, &K.smooth, [0, 2, 0], [6, 3, 5]);
    c.fill(&S.air, [1, 3, 5], 2..=3, 0);
    c.solid(&K.smooth, [1, 2, 2], [5, 3, 2]);
    c.door(JUNGLE_DOOR, [1, 2, 2], North, Hinge::Right);
    c.door(JUNGLE_DOOR, [3, 2, 2], North, Hinge::Left);
    c.door(JUNGLE_DOOR, [5, 2, 2], North, Hinge::Left);
    c.entrance([3, 2, 0], EMPTY, NOTHING);
    c.fill(&wall_torch(North), [2, 4], 3, 1);
    c.solid(&K.cut, [0, 4, 0], [6, 4, 5]);
    c.solid(&K.smooth, [0, 4, 1], [5, 4, 4]);
    c.solid(&S.air, [2, 4, 4], [4, 4, 4]);
    for step in 0..3 {
        c.place(&stairs(SMOOTH_STAIRS, East), 3 + step, 2 + step, 4);
    }
    c.solid(&K.smooth, [4, 2, 4], [5, 2, 4]);
    c.place(&K.smooth, 5, 3, 4);
    c.place(&block("minecraft:water_cauldron[level=3]"), 1, 2, 4);
    c.place(&wall_torch(East), 1, 3, 4);

    c.walls(&K.smooth, &K.smooth, [0, 5, 2], [6, 7, 5]);
    c.fill(&K.slab, [1, 3, 5], 5, 5);
    c.fill(&sandstone_slab("top"), [1, 3, 5], 6, 5);
    c.solid(&K.cut, [0, 7, 2], [6, 7, 2]);
    c.solid(&K.cut, [6, 7, 3], [6, 7, 5]);
    c.solid(&K.smooth, [0, 8, 2], [6, 8, 5]);
    c.fill(&K.terracotta, [0, 2, 4, 6], 8, [2, 5]);
    c.walls(&K.smooth, &K.terracotta, [1, 9, 2], [5, 9, 5]);
    c.fill(&K.terracotta, 3, 9, [2, 5]);
    c.open_door(
        keys::block::JUNGLE_DOOR.as_static_str(),
        [2, 5, 2],
        West,
        Hinge::Left,
    );
    c.door(JUNGLE_DOOR, [4, 5, 2], North, Hinge::Right);
    c.each(&K.potted_cactus, &[[5, 5, 0], [1, 5, 1]]);
    c.fill(&S.wall_torch, [1, 5], 6, 1);
    c.place(&wall_torch(East), 1, 6, 3);
    c.place(&wall_torch(West), 5, 6, 3);
}

fn desert_temple_1(c: &mut Canvas) {
    let (shade, lintel) = (sandstone_slab("bottom"), sandstone_slab("top"));
    c.solid(&K.cut, [0, 0, 0], [10, 0, 9]);
    c.fill(&S.air, 3..=7, 0, [0, 9]);
    c.fill(&S.air, [0, 10], 0, 3..=6);
    c.solid(&K.smooth, [1, 0, 1], [9, 0, 8]);
    c.fill(&K.cut, 3..=7, 0, [1, 8]);
    c.fill(&K.cut, 1, 0, [3, 6]);
    c.solid(&K.cut, [9, 0, 3], [9, 0, 6]);
    c.solid(&K.slab, [1, 0, 4], [2, 0, 5]);
    c.solid(&stairs(SMOOTH_STAIRS, East), [3, 0, 4], [3, 0, 5]);
    pinwheel(c, "white", [5, 0, 4]);
    c.entrance([0, 0, 5], EMPTY, NOTHING);

    c.fill(&K.smooth, 3..=7, 1..=3, [1, 8]);
    c.fill(&K.smooth, [1, 9], 1..=3, 3..=6);
    c.solid(&S.air, [1, 1, 4], [1, 1, 5]);
    c.place(&top_stairs(SMOOTH_STAIRS, North), 1, 2, 4);
    c.place(&top_stairs(SMOOTH_STAIRS, South), 1, 2, 5);
    c.solid(&shade, [3, 4, 1], [7, 4, 8]);
    c.solid(&shade, [1, 4, 3], [9, 4, 6]);
    c.fill(&K.slab, [4, 6], 4, [3, 6]);
    c.solid(&K.slab, [5, 4, 4], [5, 4, 5]);

    for x in [0, 8] {
        for z in [0, 7] {
            let side = if x == 0 { x } else { x + 2 };
            let end = if z == 0 { z } else { z + 2 };
            c.solid(&K.smooth, [x, 1, end], [x + 2, 4, end]);
            c.solid(&K.smooth, [side, 1, z], [side, 4, z + 2]);
            niches(c, &lintel, &[[x + 1, end], [side, z + 1]]);
            c.walls(&K.smooth, &K.smooth, [x, 4, z], [x + 2, 4, z + 2]);
            c.walls(&K.slab, &K.smooth, [x, 5, z], [x + 2, 5, z + 2]);
            c.place(&shade, x + 1, 5, z + 1);
            c.fill(&shade, [x, x + 2], 6, [z, z + 2]);
        }
    }
    c.place(&S.air, 1, 3, 9);
    c.brewing_stand([1, 1, 1]);
    chest(c, [1, 1, 8], East, "village_temple");
    c.fill(&S.wall_torch, [4, 6], 2, [2, 7]);
    c.fill(&wall_torch(West), 0, 2, [3, 6]);
    c.solid(&S.wall_torch, [8, 2, 4], [8, 2, 5]);
}

fn desert_temple_2(c: &mut Canvas) {
    c.solid(&K.sand, [0, 0, 0], [9, 0, 11]);
    c.solid(&K.smooth, [2, 0, 1], [4, 0, 10]);
    c.solid(&K.smooth, [5, 0, 5], [8, 0, 6]);
    c.fill(&K.smooth, 6..=7, 0, [4, 7]);
    pinwheel(c, "lime", [6, 0, 5]);
    c.entrance([0, 1, 5], EMPTY, NOTHING);
    c.brewing_stand([4, 1, 1]);
    c.solid(&K.slab, [3, 1, 10], [4, 1, 10]);

    for (south, out) in [(false, North), (true, South)] {
        let z = |from_end: i32| if south { 11 - from_end } else { from_end };
        let (step_out, step_in) = (
            stairs(SMOOTH_STAIRS, out),
            stairs(SMOOTH_STAIRS, out.opposite()),
        );
        let arch = top_stairs(SMOOTH_STAIRS, out);

        c.solid(&K.smooth, [1, 1, z(0)], [5, 3, z(0)]);
        c.fill(&K.smooth, [1, 5], 1..=3, z(2));
        c.fill(&K.smooth, 5, 1..=3, [z(1), z(3)]);
        c.solid(&K.smooth, [6, 1, z(3)], [7, 3, z(3)]);
        c.fill(&K.smooth, [1, 8], 1..=3, z(4));
        c.place(&K.smooth, 5, 3, z(4));
        c.place(&arch, 5, 2, z(4));
        c.solid(&K.smooth, [9, 1, z(5)], [9, 4, z(5)]);
        slits(c, &[[3, z(0)], [5, z(1)], [6, z(3)]]);
        c.place(&K.slab_top, 1, 3, z(1));
        c.fill(&arch, [1, 5], 3, z(5));

        c.fill(&stairs(SMOOTH_STAIRS, East), 0, 1, [z(0), z(4)]);
        c.place(&K.potted_cactus, 0, 1, z(2));
        c.fill(&K.slab, 4, 1, [z(3), z(4)]);
        c.place(&S.wall_torch, 4, 3, z(3));
        c.place(&S.wall_torch, 0, 3, z(4));
        c.place(&wall_torch(out.opposite()), 8, 3, z(5));

        c.place(&K.smooth, 1, 4, z(0));
        c.solid(&K.slab, [2, 4, z(0)], [4, 4, z(0)]);
        c.fill(&K.smooth, 1..=4, 4, [z(1), z(2)]);
        c.fill(&K.slab, 5, 4, [z(1), z(2)]);
        c.solid(&K.smooth, [1, 4, z(3)], [7, 4, z(3)]);
        c.solid(&K.smooth, [1, 4, z(4)], [5, 4, z(4)]);
        c.place(&K.smooth, 8, 4, z(4));
        c.place(&step_out, 1, 4, z(5));
        c.solid(&K.slab, [2, 4, z(5)], [3, 4, z(5)]);
        c.place(&K.slab_double, 4, 4, z(5));

        c.fill(&step_in, 1, 5, [z(0), z(3)]);
        c.fill(&step_out, 1, 5, [z(1), z(4)]);
        c.solid(&K.slab, [6, 5, z(4)], [7, 5, z(4)]);
        c.fill(&K.slab, [5, 8], 5, z(5));
        c.place(&K.slab_double, 6, 5, z(5));
        c.place(&K.slab_top, 7, 5, z(5));
    }
}

fn desert_tool_smith_1(c: &mut Canvas) {
    c.solid(&K.sand, [0, 0, 0], [8, 1, 8]);
    c.solid(&K.sand, [0, 2, 0], [2, 2, 4]);
    c.fill(&K.sand, [0, 3, 7], 2, 5);
    c.place(&K.sand, 7, 2, 6);
    c.fill(&K.sand, [1, 3, 7], 2, 7);
    c.solid(&K.sand, [2, 2, 8], [7, 2, 8]);
    c.solid(&K.smooth, [0, 2, 6], [4, 2, 6]);
    c.fill(&K.smooth, [4, 6], 2, 5..=7);
    c.place(&K.smooth, 5, 2, 7);
    c.place(&stairs(SMOOTH_STAIRS, South), 5, 1, 5);
    c.place(&stairs(SMOOTH_STAIRS, South), 5, 2, 6);

    c.solid(&K.smooth, [4, 0, 0], [7, 0, 0]);
    c.place(&K.smooth, 5, 0, 4);
    for x in [4, 6] {
        c.fill(&glazed("light_blue", South), x, 0, [1, 3]);
        c.fill(&glazed("light_blue", West), x + 1, 0, [1, 3]);
        c.place(&glazed("light_blue", East), x, 0, 2);
        c.place(&glazed("light_blue", North), x + 1, 0, 2);
    }
    c.solid(&S.air, [4, 1, 1], [7, 1, 3]);
    c.walls(&K.smooth, &K.smooth, [3, 1, 0], [8, 7, 4]);
    c.fill(&K.sand, [3, 8], 1..=2, 4);
    c.solid(&K.smooth, [3, 4, 0], [8, 4, 4]);
    c.solid(&K.terracotta, [4, 6, 0], [7, 6, 0]);
    c.solid(&K.terracotta, [8, 6, 1], [8, 6, 3]);
    c.solid(&K.slab_double, [4, 7, 0], [7, 7, 0]);
    c.solid(&K.slab_top, [4, 7, 1], [7, 7, 3]);
    c.fill(&K.slab, [3, 5, 6, 8], 8, [0, 4]);
    c.fill(&K.slab, [3, 8], 8, 2);
    c.solid(&button(South), [4, 2, 1], [7, 2, 1]);
    c.solid(&button(East), [4, 2, 2], [4, 2, 3]);
    c.solid(&button(West), [7, 2, 2], [7, 2, 3]);
    c.place(&button(North), 6, 2, 3);
    c.fill(&S.wall_torch, [4, 7], 3, 2);

    c.walls(&K.smooth, &K.smooth, [3, 3, 4], [7, 5, 8]);
    c.solid(&K.slab_top, [4, 5, 5], [6, 5, 7]);
    c.solid(&S.air, [5, 1, 4], [5, 3, 4]);
    c.fill(&K.slab, [3, 7], 6, [6, 8]);
    c.place(&K.slab, 5, 6, 8);
    c.door(JUNGLE_DOOR, [3, 3, 6], West, Hinge::Left);
    c.entrance([0, 3, 5], EMPTY, NOTHING);
    c.fill(&S.wall_torch, 2, 4, [5, 7]);
    c.place(&S.wall_torch, 6, 4, 6);
    c.each(&K.potted_cactus, &[[1, 3, 7], [2, 3, 8]]);

    c.solid(&K.smooth, [1, 3, 1], [2, 3, 3]);
    c.solid(&stairs(SMOOTH_STAIRS, East), [0, 3, 1], [0, 3, 3]);
    c.solid(&stairs(SMOOTH_STAIRS, South), [0, 3, 0], [2, 3, 0]);
    c.solid(&stairs(SMOOTH_STAIRS, North), [0, 3, 4], [2, 3, 4]);
    c.solid(&stairs(SMOOTH_STAIRS, East), [1, 4, 1], [1, 4, 3]);
    c.place(&stairs(SMOOTH_STAIRS, South), 2, 4, 1);
    c.place(&K.smooth, 2, 4, 2);
    c.place(&stairs(SMOOTH_STAIRS, North), 2, 4, 3);
    c.door(JUNGLE_DOOR, [3, 5, 2], West, Hinge::Right);
    c.place(&block(keys::block::SMITHING_TABLE.as_static_str()), 6, 5, 1);
    chest(c, [7, 5, 1], South, "village_toolsmith");
    c.fill(&S.wall_torch, 2, 6, [1, 3]);
    c.place(&S.wall_torch, 7, 6, 2);
}

fn desert_fletcher_house_1(c: &mut Canvas) {
    let ladder = block("minecraft:ladder[facing=north,waterlogged=false]");
    c.void([0, 0, 0], [0, 0, 11]);
    c.void([1, 0, 9], [5, 0, 11]);
    c.solid(&K.sand, [1, 0, 0], [5, 0, 8]);
    c.solid(&K.smooth, [2, 0, 1], [4, 0, 8]);
    c.solid(&K.smooth, [0, 0, 3], [1, 0, 3]);
    c.solid(&K.sand, [2, 0, 9], [4, 0, 11]);
    c.solid(&K.smooth, [3, 0, 9], [3, 0, 10]);

    c.walls(&K.smooth, &K.cut, [1, 1, 0], [5, 3, 8]);
    c.solid(&S.air, [2, 1, 8], [4, 2, 8]);
    c.fill(&stairs(SMOOTH_STAIRS, South), [2, 4], 1, 8);
    c.solid(&K.slab_top, [2, 3, 1], [4, 3, 8]);
    niches(c, &K.slab_top, &[[3, 0], [5, 4], [1, 6]]);
    c.fill(&K.cut, [1, 5], 4, [0, 8]);
    c.place(&K.slab, 3, 4, 0);
    c.fill(&K.slab, [1, 5], 4, [2, 4, 6]);
    c.door(JUNGLE_DOOR, [1, 1, 3], West, Hinge::Right);
    c.entrance([0, 1, 4], EMPTY, NOTHING);
    c.fill(&K.smooth, [2, 4], 1, 1);
    c.place(
        &block(keys::block::FLETCHING_TABLE.as_static_str()),
        3,
        1,
        1,
    );
    c.each(&S.wall_torch, &[[0, 2, 2], [4, 2, 2], [4, 2, 6]]);

    c.walls(&K.smooth, &K.cut, [2, 1, 9], [4, 8, 11]);
    c.walls(&K.slab, &K.cut, [2, 9, 9], [4, 9, 11]);
    c.place(&K.slab_double, 3, 9, 10);
    c.place(&low_wall(&[]), 3, 10, 10);
    c.place(&S.torch, 3, 11, 10);
    c.door(JUNGLE_DOOR, [3, 1, 9], South, Hinge::Right);
    c.solid(&ladder, [3, 2, 10], [3, 6, 10]);
    c.place(&wall_torch(North), 3, 7, 10);
}

fn desert_farm_1(c: &mut Canvas) {
    c.void([0, 2, 0], [4, 5, 6]);
    c.walls(&K.cut, &K.cut, [0, 0, 0], [4, 0, 6]);
    c.fill(&stairs(SMOOTH_STAIRS, South), [1, 3], 0, 0);
    c.fill(&stairs(SMOOTH_STAIRS, North), [0, 1, 3, 4], 0, 6);
    c.fill(&stairs(SMOOTH_STAIRS, East), 0, 0, [0, 1, 5]);
    c.fill(&stairs(SMOOTH_STAIRS, West), 4, 0, [0, 1, 5]);
    c.entrance([0, 0, 3], EMPTY, CUT);
    c.fill(&S.farmland, 1..=3, 0, [1, 3, 5]);
    c.fill(&S.water, 1..=3, 0, [2, 4]);
    c.fill(&wheat(7), 1..=3, 1, [1, 3]);
    c.place(&wheat(0), 2, 1, 1);
    c.place(&wheat(0), 1, 1, 5);
    c.place(&wheat(7), 3, 1, 5);
    c.place(&S.dirt, 2, 0, 5);
    c.place(&S.composter, 2, 1, 5);
}

fn desert_farm_2(c: &mut Canvas) {
    let trapdoor =
        settled("minecraft:jungle_trapdoor[half=bottom,open=true,powered=false,waterlogged=false]");
    let ages = [
        [2, 0, 1, 1, 0],
        [0, 0, 1, 0, 1],
        [0, 2, 0, 0, 0],
        [1, 2, 1, 1, 1],
        [2, 0, 0, 0, 3],
    ];
    c.void([0, 0, 0], [6, 6, 9]);
    c.solid(&K.smooth, [2, 0, 3], [4, 0, 3]);
    c.solid(&stairs(SMOOTH_STAIRS, South), [1, 1, 0], [6, 1, 0]);
    c.solid(&stairs(SMOOTH_STAIRS, East), [0, 1, 0], [0, 1, 5]);
    c.solid(&stairs(SMOOTH_STAIRS, West), [6, 1, 1], [6, 1, 5]);
    c.solid(&stairs(SMOOTH_STAIRS, North), [0, 1, 6], [6, 1, 6]);
    c.entrance(
        [0, 1, 3],
        EMPTY,
        "minecraft:smooth_sandstone_stairs[facing=east]",
    );
    c.solid(&S.farmland, [1, 1, 1], [5, 1, 5]);
    c.place(&S.water, 3, 1, 3);
    for (z, row) in (1..).zip(ages) {
        for (x, age) in (1..).zip(row) {
            c.place(&wheat(age), x, 2, z);
        }
    }
    c.void([3, 2, 3], [3, 2, 3]);

    c.solid(&K.smooth, [3, 1, 6], [3, 1, 8]);
    c.solid(&stairs(SMOOTH_STAIRS, East), [2, 1, 7], [2, 1, 8]);
    c.solid(&stairs(SMOOTH_STAIRS, West), [4, 1, 7], [4, 1, 9]);
    c.solid(&stairs(SMOOTH_STAIRS, North), [2, 1, 9], [3, 1, 9]);
    c.place(&S.composter, 3, 2, 8);
    c.scatter(&trapdoor, 2, &[[3, 7], [2, 8], [4, 8], [3, 9]]);
}

fn desert_large_farm_1(c: &mut Canvas) {
    let ground = [
        3, 9, 0, 10, 1, 10, 1, 10, 0, 10, 0, 10, 0, 10, 1, 10, 1, 10, 1, 9, 2, 8, 2, 8,
    ];
    let soil = [
        2, 2, 3, 6, 6, 3, 2, 3, 4, 5, 6, 4, 1, 6, 5, 1, 1, 6, 3, 6, 6, 8, 8, 6, 1, 8, 7, 2, 6, 8,
        8, 8, 8, 2, 2, 9, 4, 4, 9, 6, 8, 9, 6, 7, 10, 7, 7, 11,
    ];
    let young: [(i32, &[[i32; 2]]); 6] = [
        (1, &[[1, 6], [2, 7]]),
        (2, &[[2, 3], [3, 7]]),
        (3, &[[1, 7], [2, 8]]),
        (4, &[[2, 5], [6, 10]]),
        (5, &[[1, 5], [3, 5], [8, 7]]),
        (6, &[[2, 4], [3, 6], [3, 8]]),
    ];
    c.void([0, 1, 0], [10, 6, 12]);
    c.solid(&K.sand, [0, 0, 0], [10, 0, 12]);
    c.rows(&K.sand, 1, 1, ground.as_chunks().0);
    c.runs(&S.farmland, 1, soil.as_chunks().0);
    c.runs(&wheat(7), 2, soil.as_chunks().0);
    for (age, cells) in young {
        c.scatter(&wheat(age), 2, cells);
    }
    c.each(&S.water, &[[2, 1, 6], [7, 1, 8]]);
    c.void([2, 2, 6], [2, 2, 6]);
    c.void([7, 2, 8], [7, 2, 8]);

    c.solid(&K.smooth, [8, 1, 2], [8, 1, 4]);
    c.solid(&K.smooth, [7, 2, 1], [9, 2, 5]);
    c.solid(&S.water, [8, 2, 2], [8, 2, 4]);
    c.place(&corner_step(South, "inner_left"), 7, 2, 1);
    c.place(&corner_step(South, "inner_right"), 9, 2, 1);
    c.place(&corner_step(East, "inner_left"), 7, 2, 5);
    c.place(&corner_step(West, "inner_right"), 9, 2, 5);
    c.solid(&S.air, [6, 3, 1], [7, 3, 1]);
    c.place(&S.air, 6, 3, 2);
    c.entrance([0, 2, 6], EMPTY, NOTHING);

    c.scatter(&hay(X), 2, &[[3, 3], [9, 6], [4, 10], [2, 12]]);
    c.scatter(
        &hay(Y),
        2,
        &[[5, 1], [10, 7], [5, 10], [3, 11], [4, 11], [5, 12]],
    );
    c.scatter(
        &hay(Z),
        2,
        &[[3, 1], [0, 2], [4, 2], [4, 4], [3, 9], [9, 9]],
    );
    c.scatter(&hay(Z), 2, &[[4, 12], [7, 12]]);
    c.place(&hay(Y), 4, 3, 2);
    c.place(&hay(Z), 4, 3, 11);
    c.scatter(&S.composter, 2, &[[4, 3], [5, 9]]);
}

/// A walled pen behind a double gate, `south` its far side.
fn pen(c: &mut Canvas, south: i32, animals: [i32; 3]) {
    let gate =
        block("minecraft:jungle_fence_gate[facing=south,in_wall=true,open=false,powered=false]");
    c.void([0, 5, 0], [9, 5, south]);
    c.solid(&K.cut, [0, 0, 1], [9, 0, south]);
    c.solid(&S.grass, [1, 0, 2], [8, 0, south - 1]);
    c.fence_ring(
        &K.stone_wall,
        &gate,
        [0, 1, 1],
        [9, south],
        &[[4, 1], [5, 1]],
    );
    c.entrance([4, 0, 0], EMPTY, STEP_SOUTH);
    c.place(&stairs(SMOOTH_STAIRS, South), 5, 0, 0);
    c.spot(animals, ANIMALS, GRASS);
}

/// Slabs on four wall posts.
fn shade(c: &mut Canvas, min: [i32; 2], max: [i32; 2]) {
    c.posts(&K.stone_wall, &[min[0], max[0]], &[min[1], max[1]], [1, 3]);
    c.solid(&K.slab, [min[0], 4, min[1]], [max[0], 4, max[1]]);
}

fn desert_animal_pen_1(c: &mut Canvas) {
    pen(c, 6, [2, 0, 3]);
    shade(c, [7, 3], [9, 6]);
    c.place(&S.dirt, 5, 0, 4);
    c.place(&wet_step(North), 4, 1, 3);
    c.place(&wet_step(East), 5, 1, 3);
    c.place(&wet_step(West), 4, 1, 4);
    c.place(&wet_step(South), 5, 1, 4);
}

fn desert_animal_pen_2(c: &mut Canvas) {
    pen(c, 7, [6, 0, 3]);
    shade(c, [0, 4], [9, 7]);
    c.place(&wet_step(West), 2, 1, 4);
    c.place(&wet_step(North), 3, 1, 4);
    c.place(&wet_step(East), 4, 1, 4);
    c.solid(&wet_step(South), [2, 1, 5], [4, 1, 5]);
}

fn meeting_point_1(c: &mut Canvas, v: Village) {
    let paving = [
        7, 13, 7, 15, 7, 15, 7, 16, 7, 16, 7, 16, 7, 15, 7, 15, 7, 13,
    ];
    let rim = Patch::clipped_rectangle([10, 2], [14, 6], 1);
    c.void([0, 1, 0], [16, 5, 8]);
    c.rows(&K.smooth, 0, 0, paving.as_chunks().0);
    c.patch(&K.sand, 0, &rim);
    c.solid(&K.smooth, [11, 0, 3], [13, 0, 5]);
    c.place(&K.sand, 12, 0, 4);
    c.patch(&K.cut, 1, &rim);
    c.solid(&S.water, [11, 1, 3], [13, 1, 5]);
    lamp(c, [12, 1, 4], 3);
    c.place(&K.potted_cactus, 11, 2, 2);
    c.bell([10, 2, 4], "floor", East);

    houses(c, v, West, 7, [2, 6]);
    street_ends(c, v, &[[12, 0], [16, 4], [12, 8]]);
    c.spot([9, 1, 6], CATS, NOTHING);
    if v.zombie {
        c.place(&S.air, 9, 3, 4);
    } else {
        c.place(&S.air, 14, 3, 5);
        c.void([0, 0, 0], [6, 0, 7]);
        c.void([14, 0, 0], [16, 0, 0]);
        c.fill(&VOID, 16, 0, [1, 2, 6, 7]);
        villagers(c, v, SMOOTH, &[[11, 0, 0], [15, 0, 1], [15, 0, 7]]);
        c.spot([9, 0, 1], IRON_GOLEM, SMOOTH);
        camel(c, [8, 0, 4], "up_south", SMOOTH);
    }
}

fn meeting_point_2(c: &mut Canvas, v: Village) {
    let paving = [
        5, 7, 1, 10, 1, 10, 1, 10, 0, 10, 0, 11, 0, 11, 1, 11, 1, 10, 1, 10, 1, 10, 4, 6,
    ];
    let absent = [[8, 9], [9, 8], [9, 8], [9, 9], [8, 5], [6, 6], [5, 8]];
    c.rows(&K.smooth, 0, 0, paving.as_chunks().0);
    c.solid(&K.sand, [3, 0, 3], [8, 0, 8]);
    for (z, [low, high]) in (5..).zip(absent) {
        c.void([0, 1, z], [low, 1, z]);
        c.void([0, 2, z], [high, 2, z]);
    }
    c.void([0, 3, 5], [9, 5, 11]);
    if v.zombie {
        let open = [
            1, 1, 6, 0, 1, 7, 0, 1, 8, 2, 1, 8, 0, 1, 9, 3, 1, 9, 4, 1, 9, 1, 2, 5, 2, 2, 5, 1, 2,
            6, 2, 2, 6, 0, 2, 7, 1, 2, 7, 1, 2, 8, 1, 2, 9, 3, 2, 9, 4, 2, 9, 5, 2, 9, 2, 2, 11,
        ];
        c.each(&S.air, open.as_chunks().0);
    } else {
        c.void([0, 1, 0], [6, 5, 3]);
        c.void([0, 1, 4], [7, 5, 4]);
        c.void([0, 0, 0], [4, 0, 0]);
        c.fill(&VOID, 0, 0, [1..=3, 7..=11]);
        c.void([1, 0, 11], [3, 0, 11]);
        c.place(&S.air, 1, 1, 10);
    }

    c.walls(&K.smooth, &K.smooth, [3, 1, 3], [8, 1, 8]);
    c.solid(&S.water, [4, 1, 4], [7, 1, 7]);
    c.solid(&block("minecraft:water[level=1]"), [5, 1, 7], [6, 1, 7]);
    c.posts(&K.cut, &[4, 7], &[4, 7], [1, 4]);
    c.walls(&K.slab, &K.cut, [4, 4, 4], [7, 4, 7]);
    c.solid(&K.slab_double, [5, 4, 5], [6, 4, 6]);
    c.fill(&S.torch, [4, 7], 5, [4, 7]);
    c.bell([3, 1, 1], "floor", South);

    street_ends(c, v, &[[6, 0], [0, 5], [11, 6], [5, 11]]);
    c.spot([1, 0, 1], CATS, SMOOTH);
    c.spot([10, 0, 3], CATS, SMOOTH);
    if !v.zombie {
        villagers(c, v, SMOOTH, &[[1, 0, 8], [4, 0, 10], [10, 0, 10]]);
        c.spot([1, 0, 10], IRON_GOLEM, SMOOTH);
        camel(c, [6, 0, 1], "up_west", SMOOTH);
    }
}

/// A ring of slabs at `top` on four wall posts.
fn stall(c: &mut Canvas, min: [i32; 2], max: [i32; 2], top: i32) {
    c.posts(
        &K.stone_wall,
        &[min[0], max[0]],
        &[min[1], max[1]],
        [1, top - 1],
    );
    c.walls(
        &K.slab,
        &K.slab,
        [min[0], top, min[1]],
        [max[0], top, max[1]],
    );
}

fn meeting_point_3(c: &mut Canvas, v: Village) {
    let ground = [
        3, 9, 1, 9, 1, 11, 0, 11, 0, 13, 0, 13, 0, 13, 0, 14, 0, 14, 0, 14, 0, 14, 0, 14, 0, 14, 0,
        11, 0, 10,
    ];
    c.void([0, 0, 0], [14, 0, 14]);
    c.rows(&K.sand, 0, 0, ground.as_chunks().0);
    c.place(&K.sand, 11, 0, 1);
    c.solid(&K.sand, [12, 0, 14], [13, 0, 14]);
    if !v.zombie {
        c.void([0, 1, 0], [14, 5, 14]);
        c.each(
            &S.air,
            &[[5, 1, 4], [6, 1, 8], [1, 2, 7], [6, 2, 9], [3, 2, 11]],
        );
        c.void([0, 0, 12], [0, 0, 14]);
        c.void([1, 0, 13], [1, 0, 14]);
        c.void([2, 0, 14], [4, 0, 14]);
        c.fill(&K.smooth, [4, 7], 0, 0..=3);
        c.fill(&K.smooth, 4..=7, 0, [1, 3]);
        c.walls(&K.smooth, &K.smooth, [1, 0, 4], [6, 0, 9]);
        c.solid(&K.smooth, [3, 0, 6], [4, 0, 7]);
        c.each(&K.smooth, &[[7, 0, 4], [0, 0, 6], [0, 0, 8]]);
        villagers(c, v, SAND, &[[8, 0, 5], [3, 0, 10], [6, 0, 10]]);
        c.spot([4, 0, 11], IRON_GOLEM, SAND);
        camel(c, [7, 0, 8], "up_north", SAND);
    }

    stall(c, [4, 0], [7, 2], 4);
    for (x, z, facing) in [(5, 0, North), (6, 0, East), (5, 2, West), (6, 2, South)] {
        c.place(&glazed("white", facing), x, 1, z);
    }
    c.place(&K.potted_cactus, 5, 2, 2);
    c.bell([6, 3, 2], if v.zombie { "floor" } else { "ceiling" }, North);
    c.place(&hay(Y), 9, 1, 0);
    c.place(&hay(X), 11, 1, 2);
    lamp(c, [0, 1, 3], 2);

    c.walls(&K.smooth, &K.cut, [2, 1, 5], [5, 1, 8]);
    c.solid(&S.water, [3, 1, 6], [4, 1, 7]);
    c.fill(&sandstone_slab("bottom"), [2, 5], 2, [5, 8]);

    stall(c, [9, 5], [13, 7], 5);
    c.place(&stairs(SMOOTH_STAIRS, East), 9, 1, 6);
    c.solid(&K.cut, [10, 1, 6], [12, 1, 6]);
    c.place(&stairs(SMOOTH_STAIRS, West), 13, 1, 6);
    c.each(
        &K.potted_cactus,
        &[[10, 2, 6], [12, 2, 6], [11, 1, 7], [14, 1, 7]],
    );
    c.each(
        &K.potted_bush,
        &[[11, 2, 6], [13, 1, 4], [10, 1, 5], [12, 1, 5], [12, 1, 7]],
    );

    stall(c, [7, 11], [10, 14], 4);
    pinwheel(c, "white", [8, 1, 12]);

    street_ends(c, v, &[[0, 7], [14, 8], [12, 14]]);
    c.spot([6, 1, 4], CATS, NOTHING);
    c.spot([9, 1, 9], CATS, NOTHING);
    decorations(c, v, SAND, &[[1, 0], [2, 12], [13, 11]]);
}

#[rustfmt::skip]
mod data {
    use super::{Fields, Tag};

    pub const CAMEL_ENTITY: Fields = &[("id", Tag::String(mcrs_minecraft_entity::keys::EntityType::Camel.as_static_str()))];
}
use data::*;

templates! {
    "desert" Desert;
    both {
        "houses/desert_medium_house_1" [6, 6, 7] medium_house_1;
        "houses/desert_medium_house_2" [11, 9, 7] medium_house_2;
        "houses/desert_small_house_1" [6, 6, 5] small_house_1;
        "houses/desert_small_house_2" [7, 6, 5] small_house_2;
        "houses/desert_small_house_3" [5, 5, 6] small_house_3;
        "houses/desert_small_house_4" [5, 5, 5] small_house_4;
        "houses/desert_small_house_5" [5, 6, 6] small_house_5;
        "houses/desert_small_house_7" [8, 5, 7] small_house_7;
        "houses/desert_small_house_8" [5, 5, 5] small_house_8;
        "streets/corner_01" [7, 2, 15] corner_01;
        "streets/corner_02" [6, 2, 6] corner_02;
        "streets/crossroad_01" [18, 2, 15] crossroad_01;
        "streets/crossroad_02" [11, 2, 11] crossroad_02;
        "streets/crossroad_03" [5, 2, 5] crossroad_03;
        "streets/square_01" [13, 2, 28] square_01;
        "streets/square_02" [16, 2, 19] square_02;
        "streets/straight_01" [15, 2, 12] straight_01;
        "streets/straight_02" [15, 2, 18] straight_02;
        "streets/straight_03" [4, 2, 3] straight_03;
        "streets/turn_01" [4, 2, 4] turn_01;
        "terminators/terminator_02" [3, 2, 3] terminator_02;
        "town_centers/desert_meeting_point_1" [17, 6, 9] meeting_point_1;
        "town_centers/desert_meeting_point_2" [12, 6, 12] meeting_point_2;
        "town_centers/desert_meeting_point_3" [15, 6, 15] meeting_point_3;
    }
    single {
        "camel_spawn" [1, 4, 2] camel_spawn;
        "desert_lamp_1" [1, 4, 1] desert_lamp_1;
        "houses/desert_animal_pen_1" [10, 6, 7] desert_animal_pen_1;
        "houses/desert_animal_pen_2" [10, 6, 8] desert_animal_pen_2;
        "houses/desert_armorer_1" [7, 7, 7] desert_armorer_1;
        "houses/desert_butcher_shop_1" [8, 5, 8] desert_butcher_shop_1;
        "houses/desert_cartographer_house_1" [7, 7, 7] desert_cartographer_house_1;
        "houses/desert_farm_1" [5, 6, 7] desert_farm_1;
        "houses/desert_farm_2" [7, 7, 10] desert_farm_2;
        "houses/desert_fisher_1" [8, 6, 11] desert_fisher_1;
        "houses/desert_fletcher_house_1" [6, 12, 12] desert_fletcher_house_1;
        "houses/desert_large_farm_1" [11, 7, 13] desert_large_farm_1;
        "houses/desert_library_1" [9, 7, 5] desert_library_1;
        "houses/desert_mason_1" [7, 5, 8] desert_mason_1;
        "houses/desert_shepherd_house_1" [11, 6, 5] desert_shepherd_house_1;
        "houses/desert_small_house_6" [6, 18, 5] |c| small_house_6(c, LIVING);
        "houses/desert_tannery_1" [7, 10, 6] desert_tannery_1;
        "houses/desert_temple_1" [11, 7, 10] desert_temple_1;
        "houses/desert_temple_2" [10, 6, 12] desert_temple_2;
        "houses/desert_tool_smith_1" [9, 9, 9] desert_tool_smith_1;
        "houses/desert_weaponsmith_1" [10, 6, 7] desert_weaponsmith_1;
        "terminators/terminator_01" [3, 2, 3] terminator_01;
        "zombie/houses/desert_small_house_6" [5, 17, 5] |c| small_house_6(c, ZOMBIE);
    }
}

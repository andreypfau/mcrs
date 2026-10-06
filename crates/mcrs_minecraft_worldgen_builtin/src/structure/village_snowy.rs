use super::*;
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_keys as keys;
use mcrs_minecraft_worldgen_structure::blueprint::{Cell, Hinge, Patch, top_stairs};

const DIORITE_STAIRS: &str = keys::block::DIORITE_STAIRS.as_static_str();

const SPRUCE_STEP: &str = "minecraft:spruce_stairs[facing=east]";
const SNOW_BLOCK: &str = keys::block::SNOW_BLOCK.as_static_str();
const PLANKS: &str = keys::block::SPRUCE_PLANKS.as_static_str();
const FENCE: &str = keys::block::SPRUCE_FENCE.as_static_str();
const GATE: &str = keys::block::SPRUCE_FENCE_GATE.as_static_str();
const LOG: &str = keys::block::STRIPPED_SPRUCE_LOG.as_static_str();
const WOOD: &str = keys::block::STRIPPED_SPRUCE_WOOD.as_static_str();

kit! {
    K;
    bare_grass: block("minecraft:grass_block[snowy=false]"),
    snow: snow(1),
    snow_block: block(SNOW_BLOCK),
    ice: block(keys::block::PACKED_ICE.as_static_str()),
    blue_ice: block(keys::block::BLUE_ICE.as_static_str()),
    log: log(LOG, Y),
    log_x: log(LOG, X),
    log_z: log(LOG, Z),
    wood: log(WOOD, Y),
    wood_x: log(WOOD, X),
    wood_z: log(WOOD, Z),
    slab: block("minecraft:spruce_slab[type=bottom,waterlogged=false]"),
    slab_double: block("minecraft:spruce_slab[type=double,waterlogged=false]"),
    diorite: block(keys::block::DIORITE.as_static_str()),
    diorite_wall: settled("minecraft:diorite_wall[waterlogged=false]"),
    lantern: block("minecraft:lantern[hanging=false,waterlogged=false]"),
    hanging_lantern: block("minecraft:lantern[hanging=true,waterlogged=false]"),
}

/// The four walls of a rectangle without its corners.
fn rounded_walls(c: &mut Canvas, wall: &Cell, min: [i32; 3], max: [i32; 3]) {
    let ys = min[1]..=max[1];
    c.fill(wall, min[0] + 1..=max[0] - 1, ys.clone(), [min[2], max[2]]);
    c.fill(wall, [min[0], max[0]], ys, min[2] + 1..=max[2] - 1);
}

/// Planks stepping up from both eaves of `gable`, too steep to climb: the
/// two lowest steps are two blocks high.
fn steep_roof(c: &mut Canvas, gable: Gable) {
    let [first, last] = gable.length;
    for step in 0..gable.levels {
        let (low, high) = match step {
            0 => (gable.y, gable.y + 1),
            1 => (gable.y + 2, gable.y + 3),
            higher => (gable.y + higher + 2, gable.y + higher + 2),
        };
        for side in [gable.span[0] + step, gable.span[1] - step] {
            if gable.ridge == Z {
                c.solid(&S.spruce_planks, [side, low, first], [side, high, last]);
            } else {
                c.solid(&S.spruce_planks, [first, low, side], [last, high, side]);
            }
        }
    }
}

/// A plank roof stepping up over z 0 to 6 from height `y`, six long from
/// `x`, with a slab under the eaves and on every step but for two gaps.
fn slabbed_roof(c: &mut Canvas, x: i32, y: i32) {
    let slabs = [x, x + 2, x + 3, x + 5];
    let gable = Gable::new(X, [0, 6], [x, x + 5], y, 4);
    c.stepped_gable(gable, &S.spruce_planks, None, Some(&S.spruce_planks));
    c.fill(&S.spruce_slab_top, slabs, y - 1, [0, 6]);
    for step in 0..4 {
        c.fill(&K.slab, slabs, y + 1 + step, [step, 6 - step]);
    }
}

/// Spruce stairs round the block of a chimney where it leaves the roof.
fn chimney_collar(c: &mut Canvas, [x, y, z]: [i32; 3]) {
    c.fill(&stairs(SPRUCE_STAIRS, East), x - 1, y, [z - 1, z + 1]);
    c.fill(&stairs(SPRUCE_STAIRS, West), x + 1, y, [z - 1, z + 1]);
    c.place(&stairs(SPRUCE_STAIRS, South), x, y, z - 1);
    c.place(&stairs(SPRUCE_STAIRS, North), x, y, z + 1);
}

fn snow(layers: i32) -> Cell {
    block(&format!("minecraft:snow[layers={layers}]"))
}

/// Snow lying deeper than the thin cover: `[x, y, z, layers]` for each pile.
fn drifts(c: &mut Canvas, piles: &[[i32; 4]]) {
    for [x, y, z, layers] in piles {
        c.place(&snow(*layers), *x, *y, *z);
    }
}

/// A fence joined to `sides` whatever stands next to it.
fn fence(sides: &[Direction]) -> Cell {
    fence_joined(keys::block::SPRUCE_FENCE.as_static_str(), sides)
}

fn farmland(moisture: i32) -> Cell {
    block(&format!("minecraft:farmland[moisture={moisture}]"))
}

/// A spruce step whose corner turns towards a neighbour that is not there
/// yet.
fn corner_step(facing: Direction, shape: &str) -> Cell {
    corner_stairs(SPRUCE_STAIRS, facing, shape)
}

/// The steps either side of an entrance, their corners turned towards the
/// step the entrance becomes.
fn stoop(c: &mut Canvas, [x, y, z]: [i32; 3]) {
    c.place(&corner_step(South, "outer_left"), x, y, z - 1);
    c.place(&corner_step(North, "outer_right"), x, y, z + 1);
    c.entrance([x, y, z], EMPTY, SPRUCE_STEP);
}

fn corner_01(c: &mut Canvas, v: Village) {
    c.void([0, 1, 1], [12, 1, 15]);
    street_ends(c, v, &[[3, 0], [0, 8]]);
    path(c, Z, 3, [0, 6]);
    for step in 0..3 {
        c.solid(&S.path, [0, 0, 7 + step], [4 - step, 0, 7 + step]);
    }
    houses(c, v, East, 4, [6, 6]);
    decorations(c, v, GRASS, &[[1, 1], [6, 3], [1, 6], [9, 9], [4, 12]]);
    c.spot([1, 0, 10], CATS, GRASS);
}

fn crossroad_02(c: &mut Canvas, v: Village) {
    street_ends(c, v, &[[8, 0], [0, 8], [15, 8], [8, 15]]);
    path(c, Z, 8, [0, 15]);
    path(c, X, 8, [0, 15]);
    for z in [3, 12] {
        c.place(&S.dirt, 9, 0, z);
        houses(c, v, East, 9, [z, z]);
    }
    houses(c, v, North, 7, [3, 3]);
    let decor = [
        [5, 1],
        [14, 2],
        [3, 5],
        [11, 5],
        [8, 8],
        [1, 11],
        [13, 12],
        [5, 13],
    ];
    decorations(c, v, GRASS, &decor);
}

fn crossroad_03(c: &mut Canvas, v: Village) {
    street_ends(c, v, &[[4, 0], [15, 8], [11, 16]]);
    path(c, Z, 4, [0, 6]);
    path(c, X, 8, [0, 15]);
    path(c, Z, 11, [10, 16]);
    c.place(&S.dirt, 11, 0, 16);
    houses(c, v, North, 7, [11, 11]);
    houses(c, v, South, 9, [5, 5]);
    decorations(c, v, GRASS, &[[4, 7], [11, 9], [8, 11], [3, 13]]);
    if v.zombie {
        street_ends(c, v, &[[0, 8]]);
    } else {
        let streets = v.pool("streets");
        c.socket_facing(
            [11, 1, 15],
            "west_up",
            "minecraft:street",
            &streets,
            NOTHING,
        );
    }
}

fn square_01(c: &mut Canvas, v: Village) {
    let path = [
        17, 19, 0, 16, 19, 1, 13, 18, 2, 12, 17, 3, 11, 17, 4, 11, 17, 5, 11, 17, 6, 12, 16, 7, 13,
        15, 8,
    ];
    c.void([0, 0, 0], [19, 0, 16]);
    c.runs(&S.path, 0, path.as_chunks().0);
    street_ends(c, v, &[[18, 0]]);
    houses(c, v, West, 11, [5, 5]);
    houses(c, v, South, 8, [14, 14]);
    decorations(c, v, GRASS, &[[14, 5]]);
    decorations(c, v, NOTHING, &[[19, 2]]);
}

fn straight_03(c: &mut Canvas, v: Village) {
    village_plains::straight_with_houses(c, v, 11, [3, 7]);
    decorations(c, v, GRASS, &[[0, 2], [0, 8]]);
}

fn straight_04(c: &mut Canvas, v: Village) {
    c.jigsaw_layer(1);
    village_plains::straight_with_houses(c, v, 9, [4, 4]);
    decorations(c, v, GRASS, &[[0, 3]]);
    c.spot([1, 0, 6], CATS, PATH);
}

fn straight_08(c: &mut Canvas, v: Village) {
    village_plains::straight_with_houses(c, v, 17, [7, 10]);
    decorations(c, v, GRASS, &[[1, 3], [1, 13]]);
}

fn turn_01(c: &mut Canvas, v: Village) {
    village_plains::turn_01_of(c, v);
    decorations(c, v, GRASS, &[[8, 1]]);
}

fn small_house_1(c: &mut Canvas, v: Village) {
    c.solid(&GROUND, [0, 0, 0], [6, 0, 5]);
    c.place(&K.bare_grass, 3, 0, 5);
    rounded_walls(c, &K.ice, [1, 1, 0], [5, 3, 4]);
    c.solid(&K.ice, [2, 4, 1], [4, 4, 3]);
    c.fill(&S.pane, [1, 5], 2, 2);
    c.door(SPRUCE_DOOR, [3, 1, 4], South, Hinge::Left);
    c.entrance([3, 1, 5], EMPTY, NOTHING);
    c.bed(BLUE_BED, [4, 1, 1], West);
    c.place(&K.log, 2, 1, 1);
    c.place(&S.torch, 2, 2, 1);
    villagers(c, v, GRASS, &[[3, 0, 2]]);

    c.place(&K.snow_block, 5, 1, 4);
    c.place(&S.short_grass, 0, 1, 5);
    drifts(c, &[[0, 1, 3, 5], [1, 1, 4, 5], [6, 1, 4, 5], [0, 1, 4, 3]]);
    drifts(c, &[[5, 1, 5, 3], [2, 1, 5, 2], [4, 1, 5, 6], [5, 2, 4, 2]]);
    c.snow_on(&["soil"]);
}

/// A wooden room under a steep roof, its door on the north, and `oven` in the
/// far corner under a cobblestone flue.
fn steep_cottage(c: &mut Canvas, oven: &str) {
    c.walls(&K.wood, &K.wood, [1, 0, 1], [5, 2, 5]);
    c.fill(&K.wood_x, [1, 5], 2, 2..=4);
    c.solid(&S.spruce_planks, [2, 0, 2], [4, 0, 4]);
    c.place(&S.cobble, 4, 0, 3);
    steep_roof(c, Gable::new(Z, [0, 6], [0, 6], 1, 4));
    c.fill(&S.spruce_slab_top, [0, 6], 1, [0, 2, 4, 6]);
    c.fill(&K.slab, [0, 6], 3, [0, 2, 4, 6]);
    c.fill(&K.slab, [1, 5], 5, 3);
    c.fill(&K.wood, 2..=4, 3..=4, [1, 5]);
    c.fill(&K.wood, 3, 5, [1, 5]);
    c.fill(&S.spruce_slab_top, [1, 5], 2, 6);
    c.fill(&S.spruce_slab_top, [2, 4], 4, [0, 6]);
    c.fill(&S.spruce_slab_top, 3, 5, [0, 6]);
    c.fill(&S.pane, [1, 5], 3, 3);
    c.place(&S.pane, 3, 3, 5);

    c.door(SPRUCE_DOOR, [3, 1, 1], South, Hinge::Left);
    c.place(&stairs(SPRUCE_STAIRS, East), 2, 0, 0);
    c.place(&stairs(SPRUCE_STAIRS, West), 4, 0, 0);
    c.entrance([3, 0, 0], EMPTY, "minecraft:spruce_stairs[facing=south]");
    c.furnace([4, 1, 4], oven, North);
    c.solid(&S.cobble_wall, [4, 2, 4], [4, 4, 4]);
    c.place(&S.cobble, 4, 5, 4);
    c.solid(&S.cobble_wall, [4, 6, 4], [4, 7, 4]);

    c.snow_on(&["BOTTOM", "spruce_planks"]);
    c.no_snow([5, 5, 4], [5, 5, 4]);
}

/// The lanterns hanging along the ridge of a shop, under the fences that
/// hold them.
fn ridge_lanterns(c: &mut Canvas) {
    c.place(&K.slab_double, 3, 5, 0);
    c.fill(&S.spruce_fence, 3, 5, [2, 4]);
    c.fill(&K.hanging_lantern, 3, 4, [0, 2, 4]);
}

fn small_house_2(c: &mut Canvas, v: Village) {
    steep_cottage(c, "furnace");
    c.bed(BLUE_BED, [2, 1, 3], South);
    c.place(&wall_torch(North), 3, 3, 0);
    c.place(&wall_torch(South), 3, 5, 2);
    c.place(&wall_torch(North), 3, 5, 4);
    villagers(c, v, PLANKS, &[[3, 0, 3]]);
    if v.zombie {
        c.each(&K.wood_z, &[[1, 2, 3], [1, 2, 4], [5, 2, 4]]);
        c.solid(
            &wall_joined(
                keys::block::COBBLESTONE_WALL.as_static_str(),
                &[East, South],
                &[],
            ),
            [4, 2, 4],
            [4, 4, 4],
        );
        c.no_snow([0, 0, 0], [0, 0, 2]);
    }
}

fn snowy_armorer_house_2(c: &mut Canvas) {
    steep_cottage(c, "blast_furnace");
    ridge_lanterns(c);
    c.open_door(SPRUCE_DOOR, [3, 1, 1], South, Hinge::Left);
    c.place(&stairs(SPRUCE_STAIRS, North), 2, 1, 2);
    c.place(&stairs(SPRUCE_STAIRS, South), 2, 1, 4);
}

fn snowy_butchers_shop_1(c: &mut Canvas) {
    let counter = block("minecraft:smooth_stone_slab[type=top,waterlogged=false]");
    steep_cottage(c, "smoker");
    ridge_lanterns(c);
    c.fill(&counter, 2..=3, 0, [2, 4]);
    c.place(&counter, 3, 0, 3);
    c.place(&S.air, 2, 0, 3);
    c.no_snow([2, 0, 3], [2, 0, 3]);
    c.place(&block(keys::block::SMOOTH_STONE.as_static_str()), 2, 1, 3);
    c.door(SPRUCE_DOOR, [3, 1, 5], North, Hinge::Right);
    c.place(&K.wood, 3, 3, 5);

    c.solid(&GROUND, [2, 0, 6], [4, 0, 7]);
    c.fill(&K.wood, [1, 5], 0, 6..=8);
    c.solid(&K.wood, [2, 0, 8], [4, 0, 8]);
    c.fill(&S.spruce_fence, [1, 5], 1, 6..=8);
    c.solid(&S.spruce_fence, [2, 1, 8], [4, 1, 8]);
    c.fill(&K.snow, [2, 4], 1, 6);
    c.spot([4, 0, 7], BUTCHER_ANIMALS, DIRT);
    c.solid(&S.spruce_fence, [3, 4, 6], [3, 4, 7]);
    c.place(&K.hanging_lantern, 3, 3, 7);
}

fn small_house_3(c: &mut Canvas, v: Village) {
    c.fill(&S.spruce_planks, [0, 5], 0..=1, 0..=5);
    c.solid(&S.spruce_planks, [1, 0, 2], [4, 0, 3]);
    c.fill(&K.wood, 1..=4, 0..=1, [1, 4]);
    c.fill(&S.spruce_planks, 1, 1, 2..=3);
    c.fill(&S.spruce_planks, [1, 4], 2, 0..=3);
    c.fill(&K.wood, [1, 4], 2, 4);
    c.fill(&S.spruce_planks, [1, 4], 3, 0..=5);
    c.fill(&K.wood, 2..=3, 2..=3, 1);
    c.fill(&K.wood, 2..=3, 3..=4, 4);
    c.solid(&S.spruce_planks, [2, 4, 0], [3, 4, 3]);
    c.solid(&K.slab, [2, 5, 0], [3, 5, 5]);
    for (wing, wall, inner, up) in [(0, 1, 2, East), (5, 4, 3, West)] {
        c.fill(&stairs(SPRUCE_STAIRS, up), wing, 2, [0, 1, 4, 5]);
        c.fill(&stairs(SPRUCE_STAIRS, up), wall, 4, [0, 1, 4, 5]);
        c.place(&top_stairs(SPRUCE_STAIRS, up.opposite()), wall, 2, 5);
        c.place(&top_stairs(SPRUCE_STAIRS, up.opposite()), inner, 4, 5);
        c.place(&K.hanging_lantern, wall, 2, 6);
        c.place(&S.spruce_fence, wall, 3, 6);
    }

    c.door(SPRUCE_DOOR, [2, 1, 4], North, Hinge::Left);
    c.door(SPRUCE_DOOR, [3, 1, 4], North, Hinge::Right);
    c.solid(&stairs(SPRUCE_STAIRS, North), [2, 0, 5], [3, 0, 5]);
    c.entrance([3, 0, 6], EMPTY, NOTHING);
    c.bed(BLUE_BED, [2, 1, 3], North);
    for z in [2, 3] {
        c.furnace([4, 1, z], "furnace", West);
    }
    c.solid(&wall_torch(North), [2, 3, 3], [3, 3, 3]);
    villagers(c, v, PLANKS, &[[3, 0, 2]]);

    c.scatter(&K.snow_block, 0, &[[6, 4], [4, 5], [6, 5], [5, 6]]);
    c.place(&K.snow_block, 6, 1, 4);
    c.each(&K.snow, &[[6, 1, 5], [0, 2, 2], [0, 2, 3], [6, 2, 4]]);
    c.each(&K.snow, &[[1, 4, 2], [1, 4, 3], [4, 4, 3]]);
    drifts(
        c,
        &[
            [4, 1, 5, 4],
            [5, 1, 6, 4],
            [5, 2, 2, 2],
            [5, 2, 3, 4],
            [4, 4, 2, 4],
        ],
    );
    if v.zombie {
        c.place(&K.wood_x, 2, 3, 1);
        c.fill(&K.snow, 1..=4, 0, 0);
        c.place(&K.snow, 6, 0, 1);
        c.fill(&S.short_grass, 6, 0, [0, 2, 6]);
        drifts(
            c,
            &[
                [6, 0, 3, 7],
                [1, 0, 5, 6],
                [0, 0, 6, 5],
                [1, 0, 6, 4],
                [4, 0, 6, 7],
            ],
        );
    }
}

fn small_house_4(c: &mut Canvas, v: Village) {
    c.solid(&GROUND, [0, 0, 0], [7, 0, 6]);
    for (z, y) in [(1, 1), (2, 3), (4, 3), (5, 1)] {
        c.solid(&K.snow_block, [1, y, z], [4, y, z]);
    }
    c.solid(&K.snow_block, [2, 2, 1], [4, 2, 1]);
    c.solid(&K.snow_block, [2, 2, 5], [4, 2, 5]);
    c.fill(&K.snow_block, [1, 5, 6], 1..=2, [2, 4]);
    c.solid(&K.snow_block, [7, 1, 3], [7, 2, 3]);
    c.solid(&K.snow_block, [1, 3, 3], [6, 3, 3]);
    c.solid(&K.snow_block, [2, 4, 3], [4, 4, 3]);
    c.place(&K.snow_block, 3, 1, 0);

    c.door(SPRUCE_DOOR, [1, 1, 3], West, Hinge::Right);
    c.entrance([0, 1, 3], EMPTY, NOTHING);
    c.bed(WHITE_BED, [3, 1, 2], East);
    c.furnace([3, 1, 4], "furnace", North);
    c.fill(&stairs(SPRUCE_STAIRS, South), [2, 4], 1, 4);
    c.fill(&wall_torch(East), 2, 2, [2, 4]);
    c.place(&K.hanging_lantern, 6, 2, 3);
    villagers(c, v, GRASS, &[[4, 0, 3]]);

    c.runs(&K.snow, 3, &[[5, 6, 2], [7, 7, 3], [5, 6, 4], [2, 4, 5]]);
    c.scatter(&K.snow, 4, &[[4, 2], [6, 3], [1, 4]]);
    drifts(
        c,
        &[
            [2, 1, 0, 6],
            [4, 1, 0, 5],
            [5, 1, 1, 7],
            [6, 1, 1, 4],
            [0, 1, 2, 4],
        ],
    );
    drifts(c, &[[7, 1, 2, 8], [0, 1, 4, 8], [6, 1, 5, 6], [3, 1, 6, 5]]);
    drifts(c, &[[1, 2, 5, 2], [3, 4, 2, 3]]);
    c.snow_on(&["soil"]);
}

fn small_house_5(c: &mut Canvas, v: Village) {
    c.boxes(&GROUND, &[([1, 0, 0], [6, 0, 4]), ([0, 0, 1], [0, 0, 3])]);
    c.fill(&K.snow_block, 1..=4, 1..=2, [0, 4]);
    c.fill(&K.snow_block, 2..=3, 3, [0, 4]);
    c.fill(&K.blue_ice, [0, 5], 1..=2, [1, 3]);
    c.fill(&K.blue_ice, [1, 4], 3, [1, 3]);
    c.solid(&K.blue_ice, [6, 1, 2], [6, 2, 2]);
    c.fill(&K.snow_block, [0, 5], 3, 2);
    c.solid(&K.snow_block, [1, 4, 2], [4, 4, 2]);
    c.fill(&K.snow_block, 2..=3, 4, [1, 3]);

    c.door(SPRUCE_DOOR, [0, 1, 2], West, Hinge::Right);
    c.entrance([0, 1, 3], EMPTY, keys::block::BLUE_ICE.as_static_str());
    c.bed(RED_BED, [3, 1, 1], East);
    chest(c, [4, 1, 3], West, "village_snowy_house");
    c.place(&wall_torch(West), 5, 2, 2);
    c.place(&wall_torch(East), 1, 3, 2);
    villagers(c, v, GRASS, &[[2, 0, 2]]);
    c.snow_on(&["soil"]);
}

/// The log floor and end walls of a room five deep from x 1 to `east`, under
/// a steep roof whose ridge runs along x.
fn cabin(c: &mut Canvas, east: i32) {
    c.solid(&K.log, [1, 0, 1], [east, 0, 5]);
    c.fill(&K.log, [1, east], 1..=2, 1..=5);
    steep_roof(c, Gable::new(X, [0, 6], [0, east + 1], 1, 4));
    c.fill(&K.log, [1, east], 3..=4, 2..=4);
    c.fill(&K.log, [1, east], 5, 3);
}

fn small_house_6(c: &mut Canvas, v: Village) {
    cabin(c, 5);
    c.fill(&S.spruce_planks, [2, 4], 0, [2, 4]);
    c.fill(&K.log, [2, 4], 1..=2, [1, 5]);
    c.fill(&K.log_z, 3, 1..=2, [1, 5]);
    c.fill(&K.log_x, [1, 5], 1..=4, 3);
    c.fill(&K.log_x, 1, 0..=3, [2, 4]);
    c.fill(&K.log_x, 5, 2, [2, 4]);
    c.place(&S.pane, 5, 2, 3);

    c.door(SPRUCE_DOOR, [1, 1, 3], East, Hinge::Left);
    stoop(c, [0, 0, 3]);
    c.bed(BLUE_BED, [3, 1, 4], East);
    chest(c, [4, 1, 2], West, "village_snowy_house");
    c.furnace([2, 1, 2], "furnace", South);
    c.solid(
        &wall_joined(
            keys::block::COBBLESTONE_WALL.as_static_str(),
            &[North, West],
            &[],
        ),
        [2, 2, 2],
        [2, 4, 2],
    );
    c.place(&S.cobble, 2, 5, 2);
    c.solid(&S.cobble_wall, [2, 6, 2], [2, 7, 2]);
    c.place(&wall_torch(East), 2, 4, 3);
    c.place(&wall_torch(West), 4, 4, 3);
    c.place(&K.hanging_lantern, 0, 5, 3);
    villagers(c, v, PLANKS, &[[3, 0, 3]]);

    c.each(&S.short_grass, &[[6, 0, 3], [1, 0, 6]]);
    c.snow_on(&["BOTTOM"]);
}

fn small_house_7(c: &mut Canvas, v: Village) {
    c.solid(&K.log, [1, 0, 1], [4, 0, 5]);
    c.walls(&K.log, &K.log, [1, 1, 1], [4, 2, 5]);
    c.fill(&K.log, [1, 4], 3, 2..=4);
    c.fill(&K.log, [1, 4], 4, 3);
    c.fill(&K.log_x, 4, 1..=3, 3);
    c.fill(&K.log_x, 4, 2, [2, 4]);
    c.place(&S.pane, 4, 2, 3);
    slabbed_roof(c, 0, 2);

    c.door(SPRUCE_DOOR, [1, 1, 3], East, Hinge::Left);
    stoop(c, [0, 0, 3]);
    for z in [1, 5] {
        c.place(&S.spruce_fence, 0, 0, z);
        c.place(&K.lantern, 0, 1, z);
    }
    c.bed(WHITE_BED, [2, 1, 2], East);
    c.solid(&stairs(SPRUCE_STAIRS, South), [2, 1, 4], [3, 1, 4]);
    c.place(&K.hanging_lantern, 3, 4, 3);
    villagers(c, v, LOG, &[[2, 0, 3]]);

    c.fill(&K.snow, 2..=5, 0, [0, 6]);
    c.fill(&K.snow, 5, 0, 1..=5);
}

/// The abandoned one has sunk a layer: it stands on its walls, without the
/// floor.
fn small_house_8(c: &mut Canvas, v: Village) {
    let y = if v.zombie { 0 } else { 1 };
    rounded_walls(c, &K.snow_block, [0, y, 0], [4, y + 1, 4]);
    c.solid(&K.snow_block, [5, y, 2], [5, y + 1, 2]);
    c.solid(&K.snow_block, [0, y + 2, 1], [4, y + 2, 3]);
    c.solid(&K.snow_block, [1, y + 3, 2], [3, y + 3, 2]);
    c.entrance([0, y, 1], EMPTY, SNOW_BLOCK);
    c.bed(WHITE_BED, [2, y, 3], East);
    c.place(&stairs(SPRUCE_STAIRS, West), 1, y, 1);
    c.place(&K.log, 2, y, 1);
    c.place(&stairs(SPRUCE_STAIRS, East), 3, y, 1);
    c.place(&S.torch, 2, y + 1, 1);
    if v.zombie {
        c.door(SPRUCE_DOOR, [0, y, 2], West, Hinge::Left);
        c.snow_on(&["BOTTOM"]);
        c.no_snow([1, 0, 2], [3, 0, 3]);
    } else {
        let wool = keys::block::LIGHT_GRAY_WOOL.as_static_str();
        c.door(SPRUCE_DOOR, [0, y, 2], East, Hinge::Right);
        rounded_walls(c, &S.dirt, [0, 0, 0], [4, 0, 4]);
        c.place(&S.dirt, 5, 0, 2);
        c.solid(&block(wool), [0, 0, 2], [3, 0, 2]);
        c.fill(&block(wool), 1..=3, 0, [1, 3]);
        villagers(c, v, wool, &[[2, 0, 2]]);
    }
}

fn medium_house_1(c: &mut Canvas, v: Village) {
    c.solid(&GROUND, [0, 0, 0], [6, 0, 7]);
    c.place(&K.bare_grass, 0, 0, 0);
    c.fill(&K.snow_block, 2..=4, 1..=2, [0, 6]);
    c.fill(&K.snow_block, [1, 4, 5], 1..=2, [1, 5]);
    c.fill(&K.snow_block, [0, 6], 1..=3, 2..=4);
    c.fill(&K.snow_block, 1..=5, 3, [1, 5]);
    c.solid(&K.snow_block, [1, 4, 2], [5, 4, 4]);
    c.door(SPRUCE_DOOR, [6, 1, 3], East, Hinge::Right);
    c.entrance([6, 1, 4], EMPTY, SNOW_BLOCK);
    for z in [2, 4] {
        c.bed(WHITE_BED, [2, 1, z], West);
        villagers(c, v, GRASS, &[[3, 0, z]]);
    }
    c.place(&K.hanging_lantern, 2, 3, 3);
    c.place(&wall_torch(West), 5, 3, 3);

    c.snow_on(&["snow_block", "soil"]);
    c.no_snow([3, 3, 6], [4, 3, 6]);
    c.no_snow([3, 4, 5], [5, 4, 5]);
    c.no_snow([5, 5, 4], [5, 5, 4]);
    for z in [2, 4] {
        c.no_snow([6, 4, z], [6, 4, z]);
    }
}

fn medium_house_2(c: &mut Canvas, v: Village) {
    c.walls(&K.wood, &K.wood, [3, 0, 1], [11, 6, 4]);
    c.solid(&S.spruce_planks, [4, 0, 2], [10, 0, 3]);
    c.each(&K.wood_x, &[[3, 1, 3], [3, 3, 4], [3, 4, 2], [3, 4, 3]]);
    c.fill(&K.wood_z, [5, 9], 3, 1);
    c.fill(&S.pane, [4, 6, 8, 10], 2, 4);
    steep_roof(c, Gable::new(X, [0, 5], [2, 12], 3, 3));
    c.fill(&S.spruce_slab_top, [2, 4, 6, 8, 10, 12], 3, [0, 5]);
    c.fill(&K.slab, [3, 5, 6, 8, 9, 11], 5, [0, 5]);
    c.fill(&S.pane, [4, 7, 10], 5, [1, 4]);
    c.fill(&K.slab, [4, 7, 10], 7, [1, 4]);
    c.fill(&S.spruce_slab_top, 12, 4, [1, 4]);
    c.fill(&S.spruce_slab_top, 12, 6, [2, 3]);

    c.solid(&S.spruce_slab_top, [4, 3, 3], [10, 3, 3]);
    c.fill(&S.spruce_slab_top, [4, 5, 10], 3, 2);
    for step in 0..3 {
        c.place(&stairs(SPRUCE_STAIRS, East), 7 + step, 1 + step, 2);
    }
    c.place(&S.spruce_planks, 8, 1, 2);
    c.door(SPRUCE_DOOR, [5, 1, 1], South, Hinge::Left);
    c.door(SPRUCE_DOOR, [9, 1, 1], South, Hinge::Right);
    c.fill(&stairs(SPRUCE_STAIRS, South), [5, 9], 0, 0);
    c.entrance([7, 0, 0], EMPTY, SNOW_BLOCK);
    c.bed(RED_BED, [4, 1, 2], South);
    c.bed(BLUE_BED, [10, 1, 2], South);
    c.furnace([4, 4, 3], "furnace", East);
    c.place(&stairs(SPRUCE_STAIRS, West), 4, 4, 2);
    c.place(&wall_torch(North), 7, 2, 3);
    c.fill(&wall_torch(East), 4, 5, [2, 3]);
    c.fill(&wall_torch(West), 10, 5, [2, 3]);
    villagers(c, v, PLANKS, &[[5, 0, 3], [9, 0, 3]]);

    let blocks = [
        12, 12, 0, 1, 1, 1, 12, 13, 1, 2, 2, 2, 12, 12, 2, 1, 2, 3, 12, 13, 3, 2, 2, 4, 12, 12, 4,
        7, 8, 5, 13, 13, 5,
    ];
    c.runs(&K.snow_block, 0, blocks.as_chunks().0);
    c.each(&K.snow_block, &[[2, 1, 2], [12, 1, 3], [7, 1, 5]]);
    c.each(&K.snow, &[[1, 1, 3], [13, 1, 5]]);
    drifts(
        c,
        &[
            [8, 0, 0, 2],
            [2, 0, 5, 2],
            [11, 0, 5, 2],
            [13, 0, 6, 2],
            [8, 1, 5, 2],
        ],
    );
    drifts(
        c,
        &[
            [5, 0, 5, 3],
            [6, 0, 6, 3],
            [2, 0, 0, 4],
            [4, 0, 0, 4],
            [6, 0, 0, 4],
        ],
    );
    drifts(
        c,
        &[
            [1, 0, 5, 4],
            [8, 0, 6, 4],
            [13, 1, 3, 4],
            [2, 1, 4, 4],
            [1, 0, 2, 5],
        ],
    );
    drifts(c, &[[0, 0, 3, 5], [9, 0, 5, 5], [3, 0, 5, 6], [6, 0, 5, 6]]);
    c.snow_on(&["BOTTOM", "spruce_planks"]);
    c.no_snow([10, 5, 5], [10, 5, 5]);
    if v.zombie {
        c.fill(&K.wood_x, [3, 5, 7], 2, 4);
        c.fill(&K.wood_x, [5, 9], 3, 1);
        c.fill(&K.wood_x, [4..=7, 10..=10], 3, 4);
        c.fill(&K.wood_x, 7..=8, 4, 1);
        c.fill(&K.wood_x, [6, 10], 4, 4);
        c.fill(&K.wood_z, 11, 6, 2..=3);
        c.each(
            &K.snow_block,
            &[[12, 1, 1], [12, 1, 2], [2, 1, 3], [12, 2, 2], [2, 2, 3]],
        );
        c.place(&K.snow, 1, 1, 1);
        drifts(
            c,
            &[
                [12, 2, 3, 2],
                [2, 1, 1, 3],
                [12, 1, 4, 3],
                [12, 0, 5, 4],
                [2, 2, 2, 4],
            ],
        );
        drifts(
            c,
            &[
                [12, 3, 2, 4],
                [2, 4, 3, 4],
                [12, 2, 1, 5],
                [13, 0, 2, 6],
                [3, 0, 0, 7],
            ],
        );
        drifts(c, &[[2, 0, 1, 8], [2, 3, 3, 8]]);
        for bare in [[10, 0, 0], [2, 5, 0], [4, 5, 0], [10, 5, 0], [4, 5, 5]] {
            c.no_snow(bare, bare);
        }
    } else {
        c.place(&wall_torch(North), 5, 2, 3);
        c.fill(&VOID, 4, 5, [0, 5]);
        c.each(&K.snow_block, &[[3, 0, 0], [2, 0, 1]]);
        c.each(&K.snow, &[[12, 1, 0], [13, 1, 2], [12, 1, 4]]);
        drifts(
            c,
            &[
                [10, 0, 0, 2],
                [13, 0, 0, 2],
                [9, 0, 6, 2],
                [10, 0, 6, 2],
                [12, 0, 6, 2],
            ],
        );
        drifts(
            c,
            &[
                [2, 1, 1, 2],
                [2, 1, 3, 2],
                [2, 7, 1, 2],
                [3, 7, 4, 2],
                [5, 7, 4, 2],
            ],
        );
        drifts(
            c,
            &[
                [1, 0, 0, 3],
                [11, 0, 0, 4],
                [12, 1, 1, 4],
                [13, 0, 4, 5],
                [12, 0, 5, 5],
            ],
        );
        drifts(c, &[[12, 1, 2, 5], [13, 0, 2, 8]]);
    }
}

fn medium_house_3(c: &mut Canvas, v: Village) {
    c.solid(&GROUND, [0, 0, 0], [4, 0, 6]);
    for (sign, out) in [(-1, North), (1, South)] {
        let at = |distance: i32| 3 + sign * distance;
        c.fill(&K.blue_ice, [1, 3], 1..=2, at(3));
        c.solid(&K.ice, [2, 1, at(3)], [2, 3, at(3)]);
        c.fill(&K.blue_ice, [0, 4], 1..=2, at(2));
        c.fill(&K.blue_ice, [1, 3], 3, at(2));
        c.place(&K.ice, 2, 3, at(2));
        c.fill(&K.ice, [0, 4], 1..=2, at(1));
        c.solid(&K.ice, [0, 3, at(1)], [4, 3, at(1)]);
        c.place(&K.blue_ice, 2, 4, at(1));
        c.bed(BLUE_BED, [2, 1, at(2)], East);
        c.place(&wall_torch(out.opposite()), 2, 2, at(2));
        villagers(c, v, GRASS, &[[2, 0, at(1)]]);
    }
    c.solid(&K.ice, [4, 1, 3], [4, 3, 3]);
    c.place(&K.ice, 0, 3, 3);
    c.fill(&K.blue_ice, [1, 3], 4, 3);
    c.place(&K.ice, 2, 4, 3);
    c.door(SPRUCE_DOOR, [0, 1, 3], West, Hinge::Right);
    c.entrance([0, 1, 2], EMPTY, keys::block::PACKED_ICE.as_static_str());
    c.furnace([3, 1, 3], "furnace", West);
    c.snow_on(&["soil"]);
}

fn snowy_armorer_house_1(c: &mut Canvas) {
    let diorite = &K.diorite;
    cabin(c, 6);
    c.solid(diorite, [4, 0, 2], [5, 0, 4]);
    c.fill(&K.log_x, 1, 1..=2, [2, 4]);
    c.place(&K.log_x, 1, 3, 3);
    c.fill(&K.log_x, 6, [0, 2], [1, 3, 5]);
    c.fill(&K.log_x, 6, [1, 3], [2, 4]);
    c.place(&K.log_x, 6, 4, 3);
    c.door(SPRUCE_DOOR, [1, 1, 3], East, Hinge::Right);
    c.entrance([0, 0, 3], EMPTY, SPRUCE_STEP);

    c.fill(diorite, 5, 1, [2, 4]);
    c.solid(diorite, [5, 2, 3], [5, 6, 3]);
    c.furnace([5, 1, 3], "blast_furnace", West);
    c.place(&stairs(DIORITE_STAIRS, South), 5, 2, 2);
    c.place(&stairs(DIORITE_STAIRS, North), 5, 2, 4);
    c.place(&K.diorite_wall, 5, 7, 3);
    chimney_collar(c, [5, 6, 3]);

    chest(c, [2, 1, 5], North, "village_armorer");
    c.fill(&S.torch, 5, 1, [1, 5]);
    c.place(&wall_torch(West), 4, 4, 3);
    c.solid(&S.spruce_fence, [0, 4, 3], [0, 5, 3]);
    c.place(&K.hanging_lantern, 0, 3, 3);
    c.place(&S.short_grass, 0, 0, 5);
    c.snow_on(&["BOTTOM"]);
    for x in [2, 6] {
        c.no_snow([x, 0, 0], [x, 0, 0]);
    }
}

fn snowy_butchers_shop_2(c: &mut Canvas) {
    c.solid(&GROUND, [0, 0, 0], [8, 0, 4]);
    c.place(&S.dirt, 3, 0, 1);
    rounded_walls(c, &K.snow_block, [0, 1, 0], [4, 2, 4]);
    c.walls(&K.snow_block, &K.snow_block, [0, 3, 1], [4, 3, 3]);
    c.solid(&K.snow_block, [1, 4, 2], [3, 4, 2]);
    c.door(SPRUCE_DOOR, [0, 1, 2], West, Hinge::Right);
    c.door(SPRUCE_DOOR, [4, 1, 2], East, Hinge::Left);
    c.entrance([0, 1, 3], EMPTY, SNOW_BLOCK);
    c.solid(&S.spruce_planks, [1, 1, 1], [2, 1, 1]);
    c.place(&stairs(SPRUCE_STAIRS, North), 3, 1, 1);
    c.torch_post(&S.spruce_fence, [3, 1, 3], 1);

    c.fill(&S.spruce_fence, 5..=8, 1, [0, 4]);
    c.solid(&S.spruce_fence, [8, 1, 1], [8, 1, 3]);
    c.fill(&S.spruce_fence, 4, 1, [0, 4]);
    c.solid(&S.spruce_fence, [6, 2, 4], [6, 4, 4]);
    c.place(&S.spruce_fence, 6, 4, 3);
    c.place(&K.hanging_lantern, 6, 3, 3);
    c.furnace([7, 1, 2], "smoker", West);
    c.place(&fence(&[North, South]), 8, 1, 2);
    c.spot([5, 0, 2], BUTCHER_ANIMALS, DIRT);
    c.each(
        &K.snow,
        &[[0, 1, 0], [0, 1, 4], [1, 1, 2], [2, 1, 2], [1, 1, 3]],
    );
    c.fill(&K.snow, 5..=6, 1, 1);
}

fn snowy_cartographer_house_1(c: &mut Canvas) {
    c.solid(&K.log, [1, 0, 1], [5, 0, 9]);
    c.scatter(
        &S.spruce_planks,
        0,
        &[[2, 3], [3, 3], [1, 4], [3, 4], [1, 5]],
    );
    c.scatter(&S.spruce_planks, 0, &[[1, 6], [3, 6], [2, 7], [3, 7]]);
    for sign in [-1, 1] {
        let at = |distance: i32| 5 + sign * distance;
        c.solid(&K.log, [1, 1, at(4)], [5, 4, at(4)]);
        c.fill(&K.log_z, [2, 4], [0, 2], at(4));
        c.place(&K.log_z, 3, 1, at(4));
        c.solid(&K.log_z, [2, 3, at(4)], [4, 4, at(4)]);
        for x in [1, 5] {
            c.solid(&K.log, [x, 1, at(2)], [x, 4, at(2)]);
            c.solid(&K.log, [x, 4, at(3)], [x, 5, at(3)]);
            c.fill(&K.log_x, x, [1, 3], at(3));
            c.place(&S.pane, x, 2, at(3));
        }
        c.fill(&K.log, [2, 5], 1..=3, at(1));

        c.solid(&S.spruce_planks, [0, 3, at(5)], [6, 4, at(5)]);
        c.solid(&S.spruce_planks, [0, 5, at(4)], [6, 5, at(4)]);
        c.solid(&S.spruce_planks, [0, 6, at(3)], [6, 6, at(3)]);
        c.solid(&S.spruce_planks, [0, 5, at(2)], [6, 5, at(2)]);
        c.solid(&S.spruce_planks, [0, 4, at(1)], [6, 4, at(1)]);
        c.fill(&S.spruce_slab_top, [0, 6], 3, at(1));
        c.fill(&S.spruce_slab_top, [0, 6], 4, [at(2), at(4)]);
        c.fill(&S.spruce_slab_top, [0, 6], 5, at(3));
        c.fill(&K.slab, [0, 6], 5, at(1));
        c.place(&wall_torch(West), 1, 2, at(1));
    }
    c.solid(&S.spruce_planks, [0, 5, 5], [6, 5, 5]);
    c.fill(&S.spruce_slab_top, [0, 6], 4, 5);
    c.fill(&K.log_x, [2, 5], 3, 5);
    c.place(&K.log_x, 5, 1, 5);
    c.place(&S.pane, 5, 2, 5);
    c.place(&K.log, 2, 4, 5);
    c.place(&K.log_z, 5, 4, 5);

    c.door(SPRUCE_DOOR, [2, 1, 5], East, Hinge::Right);
    c.fill(&stairs(SPRUCE_STAIRS, East), 0, 0, [4, 6]);
    c.place(&corner_step(South, "outer_left"), 0, 0, 3);
    c.place(&corner_step(North, "outer_right"), 0, 0, 7);
    c.entrance([0, 0, 5], EMPTY, SPRUCE_STEP);
    c.place(
        &block(keys::block::CARTOGRAPHY_TABLE.as_static_str()),
        2,
        1,
        2,
    );
    chest(c, [2, 1, 3], East, "village_cartographer");
    c.solid(&stairs(SPRUCE_STAIRS, West), [2, 1, 7], [2, 1, 8]);
    c.place(&wall_torch(West), 4, 3, 5);
    c.place(&wall_torch(South), 3, 4, 2);
    c.place(&wall_torch(North), 3, 4, 8);
    c.place(&K.snow, 0, 0, 0);
    c.place(&block(keys::block::CAVE_AIR.as_static_str()), 0, 0, 1);
}

fn snowy_fisher_cottage(c: &mut Canvas) {
    c.solid(&GROUND, [0, 0, 0], [8, 0, 6]);
    c.solid(&K.bare_grass, [1, 0, 2], [2, 0, 3]);
    c.place(&S.path, 1, 0, 6);
    c.boxes(&GROUND, &[([0, 1, 1], [3, 1, 4]), ([0, 1, 5], [0, 1, 5])]);
    c.place(&S.air, 0, 1, 5);
    c.solid(&S.water, [1, 1, 2], [2, 1, 3]);
    c.solid(&S.path, [1, 1, 4], [3, 1, 4]);
    c.place(&S.path, 3, 1, 3);
    c.place(&stairs(SPRUCE_STAIRS, North), 1, 1, 5);
    c.entrance([1, 1, 6], EMPTY, NOTHING);
    c.fill(&S.spruce_fence, 0, 2..=4, [1, 4]);
    c.solid(&S.spruce_fence, [0, 4, 2], [0, 4, 3]);
    c.solid(&S.spruce_fence, [1, 4, 1], [2, 4, 1]);
    c.each(&K.hanging_lantern, &[[1, 3, 1], [0, 3, 2]]);

    c.walls(&K.log, &K.log, [4, 1, 1], [7, 3, 5]);
    c.solid(&S.spruce_planks, [5, 1, 2], [6, 1, 4]);
    slabbed_roof(c, 3, 3);
    c.fill(&K.log, [4, 7], 4, 2..=4);
    c.fill(&K.log, [4, 7], 5, 3);
    c.each(
        &K.log_x,
        &[[7, 2, 3], [7, 3, 2], [7, 3, 4], [4, 4, 3], [7, 4, 3]],
    );
    c.place(&S.pane, 7, 3, 3);
    c.door(SPRUCE_DOOR, [4, 2, 3], East, Hinge::Right);
    c.barrel([5, 2, 2], Direction::Down);
    c.barrel([6, 2, 2], Direction::Up);
    c.barrel([6, 3, 2], Direction::South);
    c.place(&K.hanging_lantern, 6, 5, 3);
    c.fill(&K.snow, [0..=0, 2..=8], 1, [0, 6]);
    c.fill(&K.snow, [0, 2, 3], 1, 5);
    c.solid(&K.snow, [8, 1, 1], [8, 1, 5]);
}

fn snowy_fletcher_house_1(c: &mut Canvas) {
    c.walls(&K.wood, &K.log, [1, 0, 1], [7, 0, 5]);
    for x in 2..=6 {
        for z in 2..=4 {
            let floor = if (x + z) % 2 == 0 {
                &S.spruce_planks
            } else {
                &K.log
            };
            c.place(floor, x, 0, z);
        }
    }
    c.fill(&K.log_x, 1, 0, [2, 4]);
    c.fill(&K.log, 1..=7, 1..=2, [1, 5]);
    c.fill(&K.log_z, [2, 4, 6], 1, [1, 5]);
    c.fill(&K.log_z, [3, 5], 2, [1, 5]);
    c.fill(&K.log_x, [1, 7], 1..=4, 2..=4);
    c.fill(&K.wood, 1, 4, [2, 4]);
    c.fill(&K.log, 7, [1, 4], [2, 4]);
    c.place(&K.log_x, 1, 5, 3);
    c.place(&K.log, 7, 5, 3);
    c.solid(&S.pane, [7, 2, 3], [7, 3, 3]);
    steep_roof(c, Gable::new(X, [0, 6], [0, 8], 1, 4));
    c.fill(&S.air, 0..=8, 1, [0, 6]);

    c.door(SPRUCE_DOOR, [1, 1, 3], East, Hinge::Left);
    stoop(c, [0, 0, 3]);
    c.place(&stairs(SPRUCE_STAIRS, West), 2, 1, 2);
    c.place(&S.spruce_fence, 3, 1, 2);
    c.place(&block(keys::block::BLUE_CARPET.as_static_str()), 3, 2, 2);
    c.place(&stairs(SPRUCE_STAIRS, East), 4, 1, 2);
    c.place(
        &block(keys::block::FLETCHING_TABLE.as_static_str()),
        6,
        1,
        3,
    );
    c.place(&wall_torch(East), 2, 4, 3);
    c.place(&wall_torch(West), 6, 4, 3);
    c.snow_on(&["BOTTOM"]);
    for x in [1, 4] {
        c.no_snow([x, 0, 6], [x, 0, 6]);
    }
}

fn snowy_library_1(c: &mut Canvas) {
    let shelf = block(keys::block::BOOKSHELF.as_static_str());
    let blocks = [
        6, 6, 0, 1, 1, 1, 11, 11, 1, 1, 1, 2, 11, 11, 2, 11, 11, 3, 1, 1, 5, 4, 6, 6, 9, 9, 6,
    ];
    let piles = [
        4, 0, 0, 2, 10, 0, 0, 2, 12, 0, 0, 2, 0, 0, 2, 2, 12, 0, 3, 2, 0, 0, 5, 2, 12, 0, 5, 2, 2,
        0, 6, 2, 11, 0, 6, 2, 3, 5, 6, 2, 1, 7, 5, 2, 4, 7, 5, 2, 3, 8, 2, 2, 7, 8, 2, 2, 3, 9, 3,
        2, 2, 0, 0, 3, 9, 0, 0, 3, 11, 0, 0, 3, 0, 0, 1, 3, 12, 0, 1, 3, 1, 0, 4, 3, 12, 0, 4, 3,
        0, 0, 6, 3, 3, 0, 6, 3, 8, 0, 6, 3, 10, 0, 6, 3, 4, 5, 6, 3, 3, 7, 5, 3, 6, 8, 2, 3, 9, 8,
        2, 3, 4, 9, 3, 3, 8, 9, 3, 3, 1, 0, 0, 4, 3, 0, 0, 4, 5, 0, 0, 4, 7, 0, 0, 4, 11, 0, 4, 4,
        1, 0, 6, 4, 7, 0, 6, 4, 11, 1, 1, 4, 1, 5, 0, 4, 3, 5, 0, 4, 5, 9, 3, 4, 11, 0, 5, 5, 2, 7,
        5, 5, 12, 0, 2, 6,
    ];
    c.walls(&K.wood, &K.wood, [2, 0, 1], [10, 6, 5]);
    c.solid(&S.spruce_planks, [3, 0, 2], [9, 0, 4]);
    c.fill(&K.wood_z, [4, 5, 7], 0, 1);
    c.place(&K.wood_z, 7, 4, 5);
    c.each(&K.wood_x, &[[2, 0, 4], [10, 2, 2], [2, 3, 1], [2, 3, 5]]);
    c.fill(&K.wood_x, [2, 10], [3, 6], 3);
    c.place(&K.wood_x, 2, 7, 3);
    c.place(&K.wood, 10, 7, 3);
    c.fill(&S.pane, [4, 6, 8], 1..=2, [1, 5]);
    c.fill(&S.pane, [2, 10], 4..=5, 3);
    c.solid(&S.pane, [10, 1, 3], [10, 2, 3]);
    steep_roof(c, Gable::new(X, [0, 6], [1, 11], 3, 4));

    c.door(SPRUCE_DOOR, [2, 1, 3], East, Hinge::Left);
    c.place(&stairs(SPRUCE_STAIRS, East), 1, 0, 3);
    c.entrance([0, 0, 3], EMPTY, NOTHING);
    c.solid(&shelf, [5, 1, 2], [5, 5, 2]);
    c.solid(&S.spruce_fence, [3, 5, 2], [9, 5, 2]);
    c.solid(&shelf, [3, 6, 2], [9, 6, 2]);
    c.place(&S.spruce_planks, 6, 6, 2);
    c.place(&shelf, 5, 5, 2);
    c.solid(&S.spruce_fence, [6, 6, 3], [6, 7, 3]);
    c.place(&K.hanging_lantern, 6, 5, 3);
    for z in [2, 4] {
        c.torch_post(&S.spruce_fence, [3, 1, z], 1);
        c.place(&S.spruce_planks, 9, 1, z);
    }
    c.place(&K.lantern, 9, 2, 4);
    c.lectern([7, 1, 3], West);

    c.runs(&K.snow_block, 0, blocks.as_chunks().0);
    c.each(
        &K.snow_block,
        &[[1, 1, 2], [11, 1, 2], [5, 1, 6], [2, 5, 0]],
    );
    drifts(c, piles.as_chunks().0);
    c.each(&K.snow, &[[4, 1, 6], [11, 1, 3], [11, 2, 2]]);
    c.snow_on(&["BOTTOM", "snow_block", "spruce_planks"]);
    c.no_snow([1, 5, 6], [1, 5, 6]);
}

fn snowy_shepherds_house_1(c: &mut Canvas) {
    c.solid(&GROUND, [1, 0, 1], [3, 0, 5]);
    c.solid(&K.log, [0, 0, 0], [3, 0, 0]);
    c.solid(&K.log, [0, 0, 1], [0, 0, 5]);
    c.solid(&S.spruce_fence, [0, 1, 0], [2, 1, 0]);
    c.solid(&S.spruce_fence, [0, 1, 1], [0, 1, 4]);
    c.place(&K.lantern, 0, 2, 0);
    c.spot(
        [1, 0, 2],
        keys::template_pool::VILLAGE_COMMON_SHEEP.as_str(),
        GRASS,
    );

    c.boxes(&K.log, &[([4, 0, 1], [7, 2, 1]), ([4, 0, 2], [4, 2, 5])]);
    c.boxes(&K.log, &[([7, 0, 2], [7, 2, 8]), ([1, 0, 6], [3, 2, 6])]);
    c.solid(&K.log, [1, 0, 8], [6, 2, 8]);
    c.place(&K.log, 1, 0, 7);
    c.boxes(
        &S.spruce_planks,
        &[([5, 0, 2], [6, 0, 5]), ([4, 0, 6], [6, 0, 6])],
    );
    c.solid(&S.spruce_planks, [2, 0, 7], [6, 0, 7]);
    c.solid(&K.log, [5, 3, 1], [6, 3, 1]);
    c.place(&K.log_z, 1, 3, 7);
    c.place(&K.log, 7, 3, 7);

    c.solid(&S.spruce_planks, [8, 1, 0], [8, 2, 5]);
    c.fill(&S.spruce_planks, [4, 7], 3, 0..=5);
    c.solid(&S.spruce_planks, [5, 4, 0], [6, 4, 6]);
    c.solid(&S.spruce_planks, [3, 1, 0], [3, 2, 0]);
    c.solid(&S.spruce_planks, [0, 1, 5], [0, 2, 5]);
    c.solid(&S.spruce_planks, [0, 1, 9], [8, 2, 9]);
    c.fill(&S.spruce_planks, 0..=8, 3, [6, 8]);
    c.solid(&S.spruce_planks, [0, 4, 7], [8, 4, 7]);
    c.solid(&K.slab, [3, 3, 0], [3, 3, 5]);
    c.solid(&K.slab, [0, 3, 5], [2, 3, 5]);
    c.fill(&K.slab, 8, 3, [0, 5, 9]);
    c.place(&K.slab, 0, 3, 9);
    c.fill(&K.slab, [4, 7], 4, 0);
    c.fill(&K.slab, [0, 8], 4, [6, 8]);

    c.door(SPRUCE_DOOR, [4, 1, 3], East, Hinge::Right);
    c.door(SPRUCE_DOOR, [1, 1, 7], East, Hinge::Left);
    c.place(&stairs(SPRUCE_STAIRS, East), 0, 0, 7);
    c.socket([0, 0, 6], "minecraft:buidling_entrance", EMPTY, NOTHING);
    c.solid(&block("minecraft:loom[facing=north]"), [5, 1, 2], [6, 1, 2]);
    chest(c, [6, 1, 7], West, "village_shepherd");
    c.place(&wall_torch(West), 6, 2, 3);
    c.place(&wall_torch(North), 5, 2, 7);
    c.each(&K.hanging_lantern, &[[2, 2, 5], [0, 3, 7]]);
    c.place(&S.short_grass, 8, 0, 8);
    c.snow_on(&["BOTTOM"]);
}

fn snowy_masons_house_1(c: &mut Canvas) {
    let blocks = [
        3, 3, 0, 6, 6, 0, 8, 8, 0, 2, 6, 1, 8, 9, 1, 8, 9, 2, 0, 0, 5, 1, 1, 6, 0, 3, 7, 6, 8, 7,
        2, 2, 8, 7, 8, 8,
    ];
    let piles = [
        0, 0, 0, 2, 1, 0, 0, 2, 5, 0, 0, 2, 7, 0, 0, 2, 0, 0, 1, 2, 0, 0, 3, 2, 0, 0, 6, 2, 9, 0,
        8, 2, 3, 1, 0, 2, 1, 5, 6, 2, 3, 6, 2, 2, 4, 7, 6, 2, 9, 0, 0, 3, 9, 0, 3, 3, 0, 0, 4, 3,
        9, 0, 5, 3, 0, 0, 8, 3, 8, 1, 8, 3, 8, 3, 7, 3, 2, 5, 1, 3, 2, 5, 7, 3, 4, 7, 5, 3, 2, 0,
        0, 4, 1, 0, 1, 4, 8, 3, 1, 4, 1, 3, 4, 4, 8, 3, 5, 4, 1, 3, 7, 4, 7, 0, 1, 5, 8, 0, 6, 5,
        9, 0, 7, 5, 3, 1, 1, 5, 2, 1, 7, 5, 1, 1, 8, 5, 2, 5, 2, 5, 4, 0, 0, 6, 1, 0, 8, 8,
    ];
    c.walls(&K.wood, &K.wood, [2, 0, 2], [7, 0, 6]);
    c.place(&K.wood_x, 2, 0, 4);
    c.fill(&K.wood, 2..=7, 1..=2, [2, 6]);
    c.fill(&K.wood, 3..=6, 3..=4, [2, 6]);
    c.fill(&K.wood, 4..=5, 5, [2, 6]);
    c.solid(&S.pane, [4, 3, 2], [5, 3, 2]);
    steep_roof(c, Gable::new(Z, [1, 8], [1, 7], 1, 4));
    c.solid(&S.spruce_planks, [2, 1, 3], [7, 1, 5]);
    c.fill(&S.spruce_planks, 6..=7, 2, [3, 5]);
    c.place(&S.spruce_planks, 7, 2, 4);

    c.open_door(SPRUCE_DOOR, [4, 2, 6], East, Hinge::Right);
    c.door(SPRUCE_DOOR, [5, 2, 6], North, Hinge::Right);
    c.solid(&stairs(SPRUCE_STAIRS, North), [4, 1, 7], [5, 1, 7]);
    c.solid(&S.spruce_planks, [4, 0, 7], [5, 0, 7]);
    c.place(&stairs(SPRUCE_STAIRS, North), 5, 0, 8);
    c.entrance([4, 0, 8], EMPTY, "minecraft:spruce_stairs[facing=north]");
    for z in 3..=5 {
        c.furnace([2, 2, z], "furnace", East);
        let (near, far) = if z == 4 {
            ("red", "blue")
        } else {
            ("blue", "red")
        };
        c.place(&block(&format!("minecraft:{near}_carpet")), 4, 2, z);
        c.place(&block(&format!("minecraft:{far}_carpet")), 5, 2, z);
    }
    c.place(&block("minecraft:stonecutter[facing=north]"), 6, 2, 4);
    c.fill(&K.hanging_lantern, [3, 6], 4, 4);

    c.runs(&K.snow_block, 0, blocks.as_chunks().0);
    c.each(
        &K.snow_block,
        &[[8, 1, 0], [2, 1, 1], [6, 1, 1], [9, 1, 1], [9, 1, 2]],
    );
    c.each(
        &K.snow_block,
        &[[0, 1, 7], [3, 1, 7], [7, 1, 7], [9, 2, 1], [1, 3, 6]],
    );
    c.each(&K.snow_block, &[[8, 3, 6], [1, 4, 6], [2, 5, 6]]);
    drifts(c, piles.as_chunks().0);
    c.snow_on(&["BOTTOM", "snow_block", "spruce_planks"]);
    for bare in [[6, 1, 0], [7, 1, 8], [9, 2, 2], [0, 2, 7], [8, 3, 4]] {
        c.no_snow(bare, bare);
    }
}

fn snowy_masons_house_2(c: &mut Canvas) {
    let diorite = &K.diorite;
    let diorite_slab = block("minecraft:diorite_slab[type=double,waterlogged=false]");
    c.fill(&K.wood, 3..=6, 0..=3, [1, 7]);
    c.fill(&K.wood, [3, 6], 0..=4, [2, 6]);
    c.fill(&K.wood, 1..=2, 0, 3..=5);
    c.fill(&K.wood, 2, 1..=3, [3, 5]);
    c.fill(&K.log_z, 4..=5, 3, [1, 7]);
    c.fill(&K.wood_z, [3, 6], 4, [3, 5]);
    c.fill(&K.wood, 3, 5, [3, 5]);
    c.fill(&K.wood_z, 6, 5, [3, 5]);
    c.place(&K.log_x, 2, 3, 4);
    c.place(&K.wood, 2, 4, 4);
    c.place(&K.wood_z, 3, 4, 4);
    c.place(&K.log_x, 3, 5, 4);
    c.place(&K.wood_z, 3, 6, 4);
    c.fill(&S.spruce_planks, 4..=5, 0, [2, 6]);
    c.solid(&S.spruce_planks, [3, 0, 3], [4, 0, 5]);

    c.fill(&S.spruce_planks, 2..=8, 2..=3, [0, 8]);
    c.fill(&K.slab, 2..=8, 4, [0, 8]);
    for y in [4, 5] {
        let gable = Gable::new(X, [1, 7], [2, 8], y, 4);
        c.stepped_gable(gable, &S.spruce_planks, None, Some(&S.spruce_planks));
    }
    c.fill(&K.slab, 2..=8, 7, [2, 6]);
    c.fill(&K.slab, 2..=8, 8, [3, 5]);
    c.place(&K.slab_double, 8, 8, 4);
    c.fill(&S.spruce_planks, 0..=2, 2..=3, [2, 6]);
    c.fill(&S.spruce_planks, 0..=2, 4, [3, 5]);
    c.solid(&S.spruce_planks, [0, 5, 4], [2, 5, 4]);
    c.solid(&K.slab, [0, 4, 2], [1, 4, 2]);
    c.solid(&K.slab, [0, 4, 6], [2, 4, 6]);
    c.fill(&K.slab, 0..=2, 5, [3, 5]);

    c.solid(diorite, [5, 0, 3], [7, 0, 5]);
    c.fill(diorite, 6..=7, 1, [3, 5]);
    c.solid(diorite, [7, 1, 3], [7, 3, 5]);
    c.solid(diorite, [7, 4, 4], [7, 6, 4]);
    c.furnace([6, 1, 4], "furnace", West);
    c.solid(
        &wall_joined(keys::block::DIORITE_WALL.as_static_str(), &[East], &[]),
        [6, 2, 4],
        [6, 3, 4],
    );
    c.solid(&diorite_slab, [6, 4, 4], [6, 6, 4]);
    c.place(diorite, 6, 7, 4);
    c.place(&diorite_slab, 6, 8, 4);
    c.place(&K.diorite_wall, 6, 9, 4);
    c.place(&stairs(DIORITE_STAIRS, South), 7, 4, 3);
    c.place(&stairs(DIORITE_STAIRS, North), 7, 4, 5);

    c.door(SPRUCE_DOOR, [2, 1, 4], East, Hinge::Right);
    c.fill(&stairs(SPRUCE_STAIRS, East), 0, 0, [3, 5]);
    c.entrance([0, 0, 4], EMPTY, SPRUCE_STEP);
    c.place(&block("minecraft:stonecutter[facing=north]"), 4, 1, 2);
    c.place(&stairs(DIORITE_STAIRS, North), 5, 1, 2);
    c.solid(&stairs(DIORITE_STAIRS, South), [4, 1, 6], [5, 1, 6]);
    c.place(&K.lantern, 6, 2, 5);
    c.fill(&K.hanging_lantern, 0, 3, [3, 5]);
    c.fill(&K.hanging_lantern, [4, 8], 6, 4);
    c.snow_on(&["BOTTOM"]);
}

fn snowy_tannery_1(c: &mut Canvas) {
    let cauldron = block("minecraft:water_cauldron[level=3]");
    c.solid(&K.log, [1, 0, 1], [6, 0, 7]);
    c.solid(&K.diorite, [3, 0, 3], [5, 0, 5]);
    c.walls(&K.log, &K.log, [1, 1, 1], [6, 2, 7]);
    steep_roof(c, Gable::new(X, [0, 8], [0, 7], 1, 5));
    c.fill(&K.log, [1, 6], 3..=4, 2..=6);
    c.fill(&K.log, [1, 6], 5, 3..=5);
    c.fill(&K.log, [1, 6], 6, 4);
    c.fill(&K.log_x, 1, 0, [1, 7]);
    c.fill(&K.log_x, 1, 1, [2, 6]);
    c.fill(&K.log_x, [1, 6], 2, [3, 5]);
    c.fill(&K.log_x, 1, 3..=6, 4);
    c.fill(&K.log_x, 1, [3, 5], [3, 5]);
    c.fill(&K.log_x, 6, 3, [2, 4, 6]);
    c.fill(&K.log_x, 6, 4, [3, 5]);
    c.fill(&S.pane, 6, 3, [3, 5]);
    c.fill(&S.spruce_slab_top, [0, 3, 4, 7], 1, [0, 8]);
    c.place(&K.slab_double, 5, 1, 8);
    for (y, step) in [(3, 0), (6, 2), (7, 3)] {
        c.fill(&K.slab, [0, 7], y, [step, 8 - step]);
    }
    c.fill(&S.spruce_slab_top, [0, 7], 4, [2, 6]);

    c.place(&K.diorite, 4, 7, 4);
    c.solid(&K.diorite_wall, [4, 2, 4], [4, 5, 4]);
    c.place(
        &wall_joined(
            keys::block::DIORITE_WALL.as_static_str(),
            &[North, South],
            &[],
        ),
        4,
        6,
        4,
    );
    c.place(&K.diorite_wall, 4, 8, 4);
    chimney_collar(c, [4, 7, 4]);
    c.furnace([4, 1, 4], "furnace", West);
    c.place(&K.log, 5, 1, 4);
    c.door(SPRUCE_DOOR, [1, 1, 4], East, Hinge::Right);
    c.entrance([0, 0, 4], EMPTY, SPRUCE_STEP);
    c.fill(&cauldron, 2, 1, [2, 6]);
    c.solid(&stairs(SPRUCE_STAIRS, North), [4, 1, 2], [5, 1, 2]);
    chest(c, [5, 1, 6], West, "village_tannery");
    c.place(&wall_torch(East), 2, 4, 4);
    c.place(&wall_torch(West), 5, 4, 4);
    c.place(&S.spruce_fence, 0, 4, 4);
    c.place(&K.hanging_lantern, 0, 3, 4);

    c.fill(&S.short_grass, 7, 0, 3..=4);
    c.scatter(&K.snow_block, 0, &[[0, 0], [4, 0], [0, 5], [0, 6]]);
    c.each(&K.snow_block, &[[0, 1, 6], [1, 3, 0], [2, 5, 7]]);
    c.place(&K.snow, 2, 3, 0);
    drifts(
        c,
        &[
            [3, 5, 7, 2],
            [0, 5, 1, 3],
            [3, 0, 0, 4],
            [5, 0, 0, 4],
            [4, 3, 0, 4],
        ],
    );
    drifts(
        c,
        &[
            [1, 5, 7, 4],
            [2, 6, 2, 4],
            [1, 0, 0, 5],
            [0, 0, 1, 5],
            [0, 0, 3, 5],
        ],
    );
    drifts(c, &[[0, 0, 7, 6]]);
    c.snow_on(&["BOTTOM"]);
}

fn snowy_temple_1(c: &mut Canvas) {
    let piles = [
        9, 0, 5, 2, 9, 1, 2, 2, 6, 3, 6, 2, 7, 5, 5, 2, 0, 6, 2, 2, 1, 6, 4, 2, 8, 6, 4, 2, 0, 7,
        3, 2, 0, 0, 0, 3, 0, 3, 0, 3, 1, 5, 1, 3, 4, 5, 5, 3, 8, 5, 5, 3, 2, 6, 2, 3, 9, 0, 3, 4,
        1, 0, 6, 4, 8, 0, 6, 4, 5, 3, 6, 4, 7, 6, 4, 4, 0, 0, 1, 5, 0, 0, 6, 5, 0, 1, 2, 5, 1, 6,
        2, 5, 0, 0, 4, 6, 8, 0, 5, 6, 8, 0, 1, 7, 9, 0, 2, 8,
    ];
    c.walls(&K.wood, &K.wood, [1, 0, 1], [7, 0, 5]);
    c.solid(&S.spruce_planks, [2, 0, 2], [6, 0, 4]);
    c.fill(&K.wood, [1, 7], 1..=2, 1..=5);
    steep_roof(c, Gable::new(X, [0, 6], [0, 8], 1, 4));
    c.fill(&K.wood, [1, 7], 3..=4, 2..=4);
    c.fill(&K.wood, [1, 7], 5, 3);
    c.each(&K.wood_x, &[[7, 1, 1], [7, 2, 1], [7, 2, 2], [7, 3, 2]]);
    c.each(&K.wood_x, &[[1, 3, 3], [1, 4, 2], [7, 4, 3], [7, 4, 4]]);
    c.solid(&stairs(SPRUCE_STAIRS, North), [2, 1, 1], [6, 1, 1]);
    c.solid(&stairs(SPRUCE_STAIRS, South), [2, 1, 5], [6, 1, 5]);
    c.door(SPRUCE_DOOR, [1, 1, 3], East, Hinge::Right);
    c.entrance([0, 0, 3], EMPTY, SPRUCE_STEP);
    c.brewing_stand([6, 1, 3]);
    c.place(&wall_torch(West), 0, 3, 3);
    c.fill(&S.spruce_fence, [2, 6], 5, 3);
    c.fill(&K.hanging_lantern, [2, 6], 4, 3);

    c.walls(&S.spruce_planks, &S.spruce_planks, [2, 10, 1], [6, 10, 5]);
    c.posts(&K.wood, &[3, 5], &[2, 4], [5, 10]);
    c.solid(&S.spruce_planks, [3, 11, 2], [5, 11, 4]);
    c.place(&S.spruce_planks, 4, 12, 3);
    c.place(&S.torch, 4, 7, 3);

    c.scatter(
        &K.snow_block,
        0,
        &[[9, 1], [0, 2], [8, 2], [8, 3], [8, 4], [9, 4], [0, 5]],
    );
    c.solid(&K.snow_block, [8, 1, 2], [8, 1, 4]);
    c.place(&K.snow_block, 8, 2, 3);
    drifts(c, piles.as_chunks().0);
    c.each(
        &K.snow,
        &[[8, 2, 2], [5, 5, 1], [6, 5, 1], [3, 5, 5], [2, 6, 4]],
    );
    c.snow_on(&["BOTTOM", "snow_block", "spruce_planks"]);
    c.no_snow([6, 0, 6], [7, 0, 6]);
    c.no_snow([7, 7, 3], [8, 7, 3]);
}

fn snowy_tool_smith_1(c: &mut Canvas) {
    let (east, north) = (stairs(SPRUCE_STAIRS, East), stairs(SPRUCE_STAIRS, North));
    c.boxes(&K.log, &[([1, 0, 1], [6, 3, 1]), ([6, 0, 2], [6, 3, 5])]);
    c.boxes(&K.log, &[([3, 0, 5], [5, 3, 5]), ([3, 0, 3], [3, 3, 4])]);
    c.solid(&K.log, [1, 0, 3], [2, 3, 3]);
    c.fill(&K.log, 1, [0, 3], 2);
    c.boxes(
        &S.spruce_planks,
        &[([2, 0, 2], [5, 0, 2]), ([4, 0, 3], [5, 0, 4])],
    );
    c.scatter(
        &K.log_z,
        3,
        &[[2, 1], [4, 1], [6, 3], [4, 5], [3, 4], [2, 3]],
    );
    c.scatter(
        &K.log_x,
        3,
        &[[3, 1], [5, 1], [6, 2], [6, 4], [5, 5], [3, 3], [1, 2]],
    );
    c.fill(&K.log, [1, 6], 4..=5, 2);
    c.solid(&K.log, [6, 4, 3], [6, 5, 4]);
    c.place(&K.log, 6, 6, 3);

    for y in 2..=3 {
        c.solid(&S.spruce_planks, [0, y, 0], [7, y, 0]);
        c.solid(&S.spruce_planks, [0, y, 4], [2, y, 4]);
        c.place(&S.spruce_planks, 2, y, 5);
        c.solid(&S.spruce_planks, [2, y, 6], [7, y, 6]);
    }
    for y in 4..=5 {
        c.solid(&S.spruce_planks, [0, y, 1], [7, y, 1]);
        c.solid(&S.spruce_planks, [0, y, 3], [3, y, 3]);
        c.place(&S.spruce_planks, 3, y, 4);
        c.solid(&S.spruce_planks, [3, y, 5], [7, y, 5]);
    }
    c.solid(&S.spruce_planks, [0, 6, 2], [7, 6, 2]);
    c.solid(&S.spruce_planks, [4, 6, 3], [5, 6, 3]);
    c.solid(&S.spruce_planks, [4, 6, 4], [7, 6, 4]);
    c.solid(&K.slab, [0, 4, 0], [7, 4, 0]);
    c.solid(&K.slab, [2, 4, 6], [7, 4, 6]);
    c.solid(&K.slab, [0, 6, 1], [7, 6, 1]);
    c.solid(&K.slab, [3, 6, 5], [7, 6, 5]);
    c.solid(&K.slab, [0, 6, 3], [1, 6, 3]);
    c.place(&K.slab, 0, 4, 4);
    c.place(&S.spruce_slab_top, 7, 6, 3);
    c.solid(&K.slab_double, [5, 7, 3], [7, 7, 3]);
    for (x, y, z) in [(1, 4, 4), (2, 6, 3)] {
        c.place(&north, x, y, z);
        c.place(&east, x + 1, y, z);
        c.place(&east, x + 1, y, z + 1);
    }

    c.door(SPRUCE_DOOR, [1, 1, 2], East, Hinge::Right);
    c.place(&corner_step(South, "outer_left"), 0, 0, 1);
    c.place(&east, 0, 0, 2);
    c.place(&corner_step(North, "outer_right"), 0, 0, 3);
    c.entrance([0, 0, 4], EMPTY, NOTHING);
    c.place(&S.spruce_planks, 4, 1, 4);
    c.place(&K.lantern, 4, 2, 4);
    c.place(&block(keys::block::SMITHING_TABLE.as_static_str()), 5, 1, 4);
    c.place(&K.hanging_lantern, 4, 5, 2);
    c.snow_on(&["BOTTOM"]);
}

fn snowy_weapon_smith_1(c: &mut Canvas) {
    let diorite = &K.diorite;
    let bars = settled("minecraft:iron_bars[waterlogged=false]");
    let grindstone = block("minecraft:grindstone[face=floor,facing=east]");
    let rail = block(
        "minecraft:diorite_wall[east=low,north=none,south=none,up=false,waterlogged=false,west=low]",
    );
    c.solid(&K.log, [1, 0, 1], [6, 0, 5]);
    c.scatter(&S.spruce_planks, 0, &[[2, 2], [3, 3], [2, 4]]);
    c.solid(diorite, [4, 0, 2], [5, 0, 4]);
    c.place(&K.log_z, 1, 0, 4);
    c.walls(&K.log, &K.log, [1, 1, 1], [6, 2, 5]);
    steep_roof(c, Gable::new(Z, [0, 7], [0, 6], 1, 4));
    c.fill(&K.log, 2..=5, 3..=4, [1, 5]);
    c.fill(&K.log, 3..=4, 5, [1, 5]);
    c.fill(&K.log_z, 2..=5, [1, 3], 1);
    c.place(&K.wood_x, 6, 1, 2);
    c.fill(&S.spruce_slab_top, [0, 7], 1, [1, 3, 5]);
    c.fill(&K.slab, [0, 7], 3, [0, 2, 4, 6]);
    c.fill(&S.spruce_slab_top, [2, 5], 4, 0);
    c.place(&S.spruce_slab_top, 2, 4, 6);
    c.fill(&S.spruce_slab_top, 3..=4, 5, [0, 6]);

    c.solid(&K.log, [6, 0, 6], [6, 2, 8]);
    c.solid(&S.spruce_planks, [7, 1, 7], [7, 2, 9]);
    c.fill(&S.spruce_slab_top, 7, 1, [7, 9]);
    c.place(&S.spruce_slab_top, 6, 2, 9);
    c.solid(&S.spruce_planks, [6, 3, 7], [6, 3, 9]);
    c.place(&S.spruce_planks, 6, 4, 7);
    c.place(&K.slab, 7, 3, 8);
    c.solid(&K.log_x, [5, 4, 6], [5, 4, 7]);
    c.solid(&stairs(SPRUCE_STAIRS, North), [5, 4, 8], [6, 4, 8]);
    c.solid(&K.slab, [5, 4, 9], [6, 4, 9]);
    c.place(&K.slab, 5, 5, 7);

    c.boxes(diorite, &[([1, 0, 6], [5, 0, 9]), ([4, 1, 5], [5, 4, 5])]);
    c.boxes(diorite, &[([3, 1, 6], [5, 3, 6]), ([5, 1, 7], [5, 3, 9])]);
    c.boxes(diorite, &[([3, 1, 7], [3, 2, 7]), ([3, 1, 9], [4, 1, 9])]);
    c.place(diorite, 3, 1, 8);
    c.solid(&block("minecraft:lava[level=0]"), [4, 1, 7], [4, 1, 8]);
    c.place(&stairs(DIORITE_STAIRS, North), 3, 3, 7);
    c.solid(&stairs(DIORITE_STAIRS, North), [3, 4, 6], [4, 4, 6]);
    c.solid(&bars, [2, 2, 9], [4, 3, 9]);
    c.place(&K.diorite_wall, 0, 1, 9);
    c.solid(&rail, [1, 1, 9], [2, 1, 9]);
    c.place(&S.torch, 0, 2, 9);
    c.place(&S.torch, 1, 1, 6);
    c.each(&grindstone, &[[5, 1, 3], [1, 1, 8]]);
    c.solid(&stairs(DIORITE_STAIRS, East), [0, 0, 7], [0, 0, 8]);

    c.door(SPRUCE_DOOR, [2, 1, 5], North, Hinge::Right);
    c.place(&K.log, 3, 1, 5);
    c.entrance([0, 0, 4], EMPTY, NOTHING);
    chest(c, [2, 1, 2], South, "village_weaponsmith");
    c.fill(&K.hanging_lantern, [2, 5], 4, 3);
    c.fill(
        &wall_joined(keys::block::DIORITE_WALL.as_static_str(), &[], &[East]),
        0,
        0,
        [6, 9],
    );
    c.snow_on(&["BOTTOM"]);
}

fn meeting_point_1(c: &mut Canvas, v: Village) {
    let path = [
        1, 3, 0, 1, 8, 1, 1, 2, 2, 7, 9, 2, 1, 2, 3, 7, 10, 3, 0, 2, 4, 7, 11, 4, 0, 2, 5, 7, 8, 5,
        10, 11, 5, 0, 11, 6, 7, 9, 7,
    ];
    let ends = [[2, 0], [0, 5], [11, 5], [8, 7]];
    c.solid(&GROUND, [0, 0, 0], [11, 0, 7]);
    c.runs(&S.path, 0, path.as_chunks().0);
    c.solid(&K.wood, [3, 1, 2], [3, 1, 4]);
    c.solid(&K.wood_x, [4, 1, 2], [6, 1, 2]);
    c.solid(&K.wood_x, [3, 1, 5], [5, 1, 5]);
    c.solid(&K.wood_z, [6, 1, 3], [6, 1, 5]);
    c.solid(&K.ice, [4, 1, 3], [5, 2, 4]);
    c.solid(&K.ice, [4, 3, 3], [4, 5, 3]);
    c.solid(&K.ice, [5, 3, 4], [5, 7, 4]);
    c.place(&K.ice, 9, 1, 5);
    c.bell([9, 2, 5], "floor", North);
    street_ends(c, v, &ends);
    decorations(c, v, GRASS, &[[10, 1], [2, 7]]);
    spots(c, CATS, GRASS, &[[4, 0, 0], [5, 0, 0], [5, 0, 7]]);
    if !v.zombie {
        c.scatter(&S.dirt, 0, &ends);
        c.spot([1, 0, 2], IRON_GOLEM, PATH);
        villagers(c, v, PATH, &[[1, 0, 4], [8, 0, 3]]);
    }
    c.snow_on(&["soil", "stripped_spruce_wood"]);
    for bare in [[9, 1, 1], [10, 1, 2], [4, 1, 7], [6, 1, 7], [10, 1, 7]] {
        c.no_snow(bare, bare);
    }
}

fn meeting_point_2(c: &mut Canvas, v: Village) {
    let path = [
        4, 6, 0, 2, 8, 1, 0, 9, 2, 0, 10, 3, 0, 10, 4, 0, 10, 5, 1, 9, 6, 2, 8, 7, 4, 6, 8,
    ];
    let steps = [
        (4, 2, South),
        (5, 2, South),
        (6, 2, West),
        (3, 3, South),
        (4, 3, East),
        (6, 3, West),
        (7, 3, South),
        (3, 4, East),
        (7, 4, West),
        (3, 5, North),
        (4, 5, East),
        (6, 5, North),
        (7, 5, West),
        (4, 6, East),
        (5, 6, North),
        (6, 6, West),
    ];
    c.void([0, 0, 0], [10, c.size()[1] - 1, 8]);
    c.runs(&S.path, 0, path.as_chunks().0);
    for (x, z, facing) in steps {
        c.place(&GROUND, x, 0, z);
        c.place(&stairs(SPRUCE_STAIRS, facing), x, 1, z);
    }
    c.place(&block(keys::block::STONE_BRICKS.as_static_str()), 4, 0, 3);
    for y in 1..=2 {
        c.patch(&K.ice, y, &Patch::diamond([5, 4], 1));
    }
    c.place(&K.ice, 5, 3, 4);
    c.place(&S.torch, 5, 4, 4);
    c.place(&S.spruce_planks, 1, 0, 4);
    c.bell([1, 1, 4], "floor", East);
    street_ends(c, v, &[[5, 0], [0, 4], [10, 4], [5, 8]]);
    spots(c, CATS, GRASS, &[[0, 0, 2], [2, 0, 0]]);
    if !v.zombie {
        c.spot([2, 0, 7], IRON_GOLEM, PATH);
        villagers(c, v, PATH, &[[1, 0, 2], [3, 0, 1]]);
        c.place(&S.air, 3, 3, 1);
    }
}

fn meeting_point_3(c: &mut Canvas, v: Village) {
    c.void([0, 0, 0], [6, 6, 6]);
    c.patch(&S.path, 0, &Patch::clipped_rectangle([0, 0], [6, 6], 2));
    c.fill(&GROUND, [2, 4], 0, [2, 4]);
    c.posts(&S.spruce_fence, &[2, 4], &[2, 4], [1, 3]);
    c.solid(&K.wood_z, [2, 4, 2], [4, 4, 4]);
    c.each(&K.wood_x, &[[3, 4, 2], [4, 4, 2], [3, 4, 4]]);
    c.place(&K.wood_z, 3, 5, 3);
    for side in [North, East, South, West] {
        let step = side.normal();
        let (x, z) = (3 + 2 * step.x, 3 + 2 * step.z);
        c.place(&S.spruce_fence, x, 4, z);
        c.place(&K.hanging_lantern, x, 3, z);
    }
    c.bell([3, 3, 3], "ceiling", East);
    street_ends(c, v, &[[3, 0], [0, 3], [6, 3], [3, 6]]);
    spots(c, CATS, GRASS, &[[0, 0, 1], [6, 0, 6]]);
    if !v.zombie {
        c.spot([1, 0, 0], IRON_GOLEM, GRASS);
        villagers(c, v, PATH, &[[1, 0, 1], [1, 0, 5]]);
    }
}

/// A fence post three high on a spot at `[x, z]`, with a fence arm at the top
/// and a lantern hanging from it on each of `sides`.
fn lamp_post(c: &mut Canvas, [x, z]: [i32; 2], sides: &[Direction]) {
    c.socket_facing([x, 0, z], "down_south", BOTTOM, EMPTY, FENCE);
    c.solid(&S.spruce_fence, [x, 1, z], [x, 3, z]);
    for side in sides {
        let step = side.normal();
        c.place(&S.spruce_fence, x + step.x, 3, z + step.z);
        c.place(&K.hanging_lantern, x + step.x, 2, z + step.z);
    }
}

fn snowy_lamp_post_03(c: &mut Canvas) {
    lamp_post(c, [1, 1], &[North, East, South, West]);
    c.scatter(&K.snow, 0, &[[1, 0], [2, 1], [1, 2]]);
}

fn snowy_animal_pen_1(c: &mut Canvas) {
    let trough = |facing| {
        settled(&format!(
            "{SPRUCE_STAIRS}[facing={facing},half=bottom,waterlogged=true]"
        ))
    };
    c.void([0, 4, 0], [7, 5, 8]);
    c.solid(&GROUND, [2, 0, 3], [6, 0, 7]);
    c.walls(&K.log, &K.log, [1, 0, 2], [7, 0, 8]);
    c.fence_ring(
        &S.spruce_fence,
        &gate(GATE, East),
        [1, 1, 2],
        [7, 8],
        &[[1, 5]],
    );
    for z in [4, 6] {
        c.torch_post(&S.spruce_fence, [1, 1, z], 2);
    }
    stoop(c, [0, 0, 5]);
    c.spot([2, 0, 5], ANIMALS, DIRT);

    c.place(&S.dirt, 5, 0, 5);
    c.place(&trough("west"), 4, 1, 5);
    c.place(&trough("north"), 5, 1, 5);
    c.place(&trough("south"), 4, 1, 6);
    c.place(&trough("east"), 5, 1, 6);

    c.solid(&K.snow_block, [3, 0, 1], [5, 0, 1]);
    c.each(&K.snow_block, &[[4, 0, 0], [4, 1, 1]]);
    drifts(c, &[[3, 0, 0, 6], [5, 0, 0, 4], [2, 0, 1, 5], [6, 0, 1, 5]]);
    drifts(c, &[[0, 0, 3, 5], [0, 0, 7, 5], [0, 0, 8, 2], [5, 1, 1, 3]]);
    c.snow_on(&["BOTTOM", "soil"]);
}

fn snowy_animal_pen_2(c: &mut Canvas) {
    c.void([0, 0, 0], [3, 0, 1]);
    c.void([8, 0, 0], [8, 0, 2]);
    c.void([0, 3, 0], [8, 5, 7]);
    c.boxes(&GROUND, &[([4, 0, 0], [7, 0, 2]), ([0, 0, 2], [3, 0, 2])]);
    c.solid(&GROUND, [0, 0, 3], [8, 0, 7]);
    c.fill(&K.bare_grass, 5, 0, 3..=4);
    c.runs(&S.water, 0, &[[7, 7, 5], [6, 7, 6]]);
    c.spot([2, 0, 4], ANIMALS, DIRT);
    c.spot([6, 0, 3], ANIMALS, NOTHING);

    c.boxes(
        &S.spruce_fence,
        &[([4, 1, 0], [7, 1, 0]), ([4, 1, 1], [4, 1, 2])],
    );
    c.boxes(
        &S.spruce_fence,
        &[([7, 1, 1], [7, 1, 3]), ([0, 1, 2], [3, 1, 2])],
    );
    c.boxes(
        &S.spruce_fence,
        &[([0, 1, 3], [0, 1, 7]), ([8, 1, 3], [8, 1, 7])],
    );
    c.solid(&S.spruce_fence, [1, 1, 7], [7, 1, 7]);
    c.place(&gate(GATE, South), 5, 1, 0);
    c.place(&fence(&[East, South]), 4, 1, 0);
    c.place(&K.lantern, 8, 2, 7);
    c.entrance([3, 1, 0], EMPTY, NOTHING);

    let cover = [
        [5, 5, 2],
        [2, 5, 3],
        [1, 1, 4],
        [5, 6, 4],
        [1, 2, 5],
        [1, 4, 6],
    ];
    c.runs(&K.snow, 1, &cover);
    c.solid(&K.snow_block, [4, 1, 4], [4, 1, 5]);
    c.each(&K.snow_block, &[[3, 1, 5], [4, 2, 5]]);
    drifts(c, &[[3, 1, 4, 3], [5, 1, 5, 4]]);
}

fn snowy_farm_1(c: &mut Canvas) {
    c.walls(&K.wood, &K.log, [0, 0, 0], [5, 0, 6]);
    c.fill(&K.wood_x, 1..=4, 0, [0, 6]);
    c.fill(&stairs(SPRUCE_STAIRS, East), 0, 0, [2, 4]);
    c.entrance([0, 0, 3], EMPTY, WOOD);
    c.solid(&farmland(7), [1, 0, 1], [4, 0, 5]);
    c.solid(&S.water, [2, 0, 3], [3, 0, 3]);
    c.fill(&wheat(0), 1..=4, 1, [1, 2, 4, 5]);
    c.fill(&wheat(0), [1, 4], 1, 3);
    c.scatter(&wheat(1), 1, &[[4, 1], [1, 2], [4, 2], [3, 5]]);
    c.solid(&S.spruce_fence, [5, 1, 3], [5, 4, 3]);
    c.solid(&S.spruce_fence, [3, 4, 3], [4, 4, 3]);
    c.place(&K.hanging_lantern, 3, 3, 3);
    c.place(&S.composter, 5, 1, 5);
}

fn snowy_farm_2(c: &mut Canvas) {
    let snow = [
        3, 3, 0, 2, 5, 1, 5, 6, 2, 6, 6, 3, 0, 0, 4, 3, 3, 4, 6, 6, 4, 1, 1, 5, 6, 6, 5, 0, 1, 6,
        5, 5, 6, 3, 5, 7, 1, 3, 8,
    ];
    let dry = [4, 4, 2, 5, 5, 3, 1, 2, 4, 5, 5, 5, 2, 2, 7];
    let wet = [
        1, 3, 2, 1, 1, 3, 4, 4, 3, 4, 5, 4, 2, 2, 5, 4, 4, 5, 2, 4, 6,
    ];
    c.void([0, 3, 0], [6, 5, 8]);
    c.runs(&K.snow_block, 0, snow.as_chunks().0);
    for (soil, runs) in [(farmland(0), &dry[..]), (farmland(7), &wet[..])] {
        c.runs(&soil, 0, runs.as_chunks().0);
        c.runs(&wheat(0), 1, runs.as_chunks().0);
    }
    c.place(&farmland(6), 2, 0, 3);
    c.place(&wheat(0), 2, 1, 3);
    c.fill(&S.water, 3, 0, [3, 5]);
    c.place(&stairs(SPRUCE_STAIRS, East), 0, 0, 2);
    c.entrance([0, 0, 3], EMPTY, SNOW_BLOCK);
    c.place(&K.lantern, 3, 1, 4);
    c.place(&S.composter, 3, 1, 7);

    c.scatter(&K.snow_block, 1, &[[3, 1], [1, 6], [5, 6], [4, 7], [3, 8]]);
    c.place(&K.snow, 0, 1, 6);
    drifts(c, &[[1, 0, 1, 5], [0, 0, 5, 4], [6, 0, 6, 4], [1, 0, 7, 4]]);
    drifts(c, &[[4, 0, 8, 5], [2, 1, 1, 2], [5, 1, 2, 2]]);
    c.snow_on(&["BOTTOM"]);
}

templates! {
    "snowy" SNOW;
    both {
        "houses/snowy_medium_house_1" [7, 6, 8] medium_house_1;
        "houses/snowy_medium_house_2" [14, 9, 7] medium_house_2;
        "houses/snowy_medium_house_3" [5, 5, 7] medium_house_3;
        "houses/snowy_small_house_1" [7, 5, 6] small_house_1;
        "houses/snowy_small_house_2" [7, 8, 7] small_house_2;
        "houses/snowy_small_house_3" [7, 6, 7] small_house_3;
        "houses/snowy_small_house_4" [8, 5, 7] small_house_4;
        "houses/snowy_small_house_5" [7, 5, 5] small_house_5;
        "houses/snowy_small_house_6" [7, 9, 7] small_house_6;
        "houses/snowy_small_house_7" [6, 7, 7] small_house_7;
        "streets/corner_01" [13, 2, 16] corner_01;
        "streets/corner_02" [16, 2, 16] village_plains::corner_02_of;
        "streets/corner_03" [4, 2, 4] village_plains::corner_03_of;
        "streets/crossroad_01" [16, 2, 16] village_plains::crossroad_01_of;
        "streets/crossroad_02" [16, 2, 16] crossroad_02;
        "streets/crossroad_03" [16, 2, 17] crossroad_03;
        "streets/crossroad_04" [4, 2, 5] village_plains::crossroad_04_of;
        "streets/crossroad_05" [5, 2, 5] village_plains::crossroad_05_of;
        "streets/crossroad_06" [5, 2, 5] village_plains::crossroad_06_of;
        "streets/square_01" [20, 2, 17] square_01;
        "streets/straight_01" [16, 2, 16] village_plains::straight_01_of;
        "streets/straight_02" [16, 2, 16] |c, v| village_plains::straight_with_houses(c, v, 16, [8, 8]);
        "streets/straight_03" [13, 2, 11] straight_03;
        "streets/straight_04" [11, 2, 9] straight_04;
        "streets/straight_06" [21, 2, 18] village_plains::straight_06_of;
        "streets/straight_08" [16, 2, 17] straight_08;
        "streets/turn_01" [18, 2, 8] turn_01;
        "town_centers/snowy_meeting_point_1" [12, 8, 8] meeting_point_1;
        "town_centers/snowy_meeting_point_3" [7, 7, 7] meeting_point_3;
    }
    single {
        "houses/snowy_animal_pen_1" [8, 6, 9] snowy_animal_pen_1;
        "houses/snowy_animal_pen_2" [9, 6, 8] snowy_animal_pen_2;
        "houses/snowy_armorer_house_1" [8, 8, 7] snowy_armorer_house_1;
        "houses/snowy_armorer_house_2" [7, 8, 7] snowy_armorer_house_2;
        "houses/snowy_butchers_shop_1" [7, 8, 9] snowy_butchers_shop_1;
        "houses/snowy_butchers_shop_2" [9, 5, 5] snowy_butchers_shop_2;
        "houses/snowy_cartographer_house_1" [7, 7, 11] snowy_cartographer_house_1;
        "houses/snowy_farm_1" [6, 6, 7] snowy_farm_1;
        "houses/snowy_farm_2" [7, 6, 9] snowy_farm_2;
        "houses/snowy_fisher_cottage" [9, 8, 7] snowy_fisher_cottage;
        "houses/snowy_fletcher_house_1" [9, 8, 7] snowy_fletcher_house_1;
        "houses/snowy_library_1" [13, 10, 7] snowy_library_1;
        "houses/snowy_masons_house_1" [10, 8, 9] snowy_masons_house_1;
        "houses/snowy_masons_house_2" [9, 10, 9] snowy_masons_house_2;
        "houses/snowy_shepherds_house_1" [9, 5, 10] snowy_shepherds_house_1;
        "houses/snowy_small_house_8" [6, 5, 5] |c| small_house_8(c, LIVING);
        "houses/snowy_tannery_1" [8, 9, 9] snowy_tannery_1;
        "houses/snowy_temple_1" [10, 14, 7] snowy_temple_1;
        "houses/snowy_tool_smith_1" [8, 8, 7] snowy_tool_smith_1;
        "houses/snowy_weapon_smith_1" [9, 7, 10] snowy_weapon_smith_1;
        "snowy_lamp_post_01" [3, 4, 1] |c| lamp_post(c, [1, 0], &[West, East]);
        "snowy_lamp_post_02" [2, 4, 1] |c| lamp_post(c, [1, 0], &[West]);
        "snowy_lamp_post_03" [3, 4, 3] snowy_lamp_post_03;
        "town_centers/snowy_meeting_point_2" [11, 5, 9] |c| meeting_point_2(c, LIVING);
        "zombie/houses/snowy_small_house_8" [6, 4, 5] |c| small_house_8(c, ZOMBIE);
        "zombie/town_centers/snowy_meeting_point_2" [11, 6, 9] |c| meeting_point_2(c, ZOMBIE);
    }
}

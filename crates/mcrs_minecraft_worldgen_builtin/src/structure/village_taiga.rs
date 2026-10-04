use super::*;
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_worldgen_structure::blueprint::{Cell, Hinge, top_stairs};

const PURPLE_BED: &str = "minecraft:purple_bed";

const PLANKS: &str = "minecraft:spruce_planks";
const SPRUCE_LOG: &str = "minecraft:spruce_log";
const TRAPDOOR: &str = "minecraft:spruce_trapdoor";
const STEP_SOUTH: &str = "minecraft:cobblestone_stairs[facing=south]";
const STEP_EAST: &str = "minecraft:cobblestone_stairs[facing=east]";

const HOUSE_LOOT: &str = "village_taiga_house";

impl Village {
    /// What an entrance standing in a doorway turns into.
    fn doorway(self) -> &'static str {
        if self.zombie { NOTHING } else { AIR }
    }

    /// The soil under a path or a step: bare, or grown over in the abandoned
    /// village.
    fn underfoot(self) -> Cell {
        if self.zombie { GROUND } else { S.dirt.clone() }
    }
}

/// An entrance facing west that stands inside the box, off its west face.
fn west_entrance(c: &mut Canvas, at: [i32; 3], step: &str) {
    c.socket_facing(at, "west_up", "minecraft:building_entrance", EMPTY, step);
}

/// A jigsaw nothing joins, turning into `floor` once its piece is placed.
fn marker(c: &mut Canvas, at: [i32; 3], floor: &str) {
    c.socket_facing(at, "down_south", BOTTOM, EMPTY, floor);
}

kit! {
    K;
    log: log(SPRUCE_LOG, Y),
    log_x: log(SPRUCE_LOG, X),
    log_z: log(SPRUCE_LOG, Z),
    fern: block("minecraft:fern"),
    large_fern: block("minecraft:large_fern[half=lower]"),
    plate: block("minecraft:spruce_pressure_plate[powered=false]"),
    gate: block("minecraft:spruce_fence_gate[facing=north,in_wall=false,open=false,powered=false]"),
    pumpkin: block("minecraft:pumpkin"),
    purple_carpet: block("minecraft:purple_carpet"),
    grindstone: block("minecraft:grindstone[face=floor,facing=south]"),
}

fn trapdoor(facing: Direction, half: &str) -> Cell {
    super::trapdoor(TRAPDOOR, facing, half, true)
}

fn shut_trapdoor(facing: Direction, half: &str) -> Cell {
    super::trapdoor(TRAPDOOR, facing, half, false)
}

/// A cobblestone wall post joined to the sides in `low` and in `tall`.
fn wall_post(low: &[Direction], tall: &[Direction]) -> Cell {
    wall_joined("minecraft:cobblestone_wall", low, tall)
}

fn campfire(c: &mut Canvas, at: [i32; 3], facing: Direction, signal: bool) {
    let state = format!(
        "minecraft:campfire[facing={},lit=true,signal_fire={signal},waterlogged=false]",
        facing.name()
    );
    c.machine(at, &state, CAMPFIRE_DATA);
}

/// A roof of logs lying along the ridge, stepping up from both eaves.
fn log_roof(c: &mut Canvas, gable: Gable) {
    let log = if gable.ridge == X { &K.log_x } else { &K.log_z };
    c.stepped_gable(gable, log, None, Some(log));
}

/// `block` under the slopes of `gable` on the line `at` of its ridge, from
/// the second level up.
fn gable_end(c: &mut Canvas, block: &Cell, gable: Gable, at: i32) {
    for i in 1..gable.levels {
        let (a, b, y) = (gable.span[0] + i + 1, gable.span[1] - i - 1, gable.y + i);
        if gable.ridge == Z {
            c.solid(block, [a, y, at], [b, y, at]);
        } else {
            c.solid(block, [at, y, a], [at, y, b]);
        }
    }
}

/// A log roof over walls of `wall` that close both gables on the lines
/// `ends` of the ridge.
fn gabled_roof(c: &mut Canvas, gable: Gable, wall: &Cell, ends: [i32; 2]) {
    for at in ends {
        gable_end(c, wall, gable, at);
    }
    log_roof(c, gable);
}

/// An open shutter hinged at `half` on the wall either side of the opening
/// at `[x, y, z]`, which looks `out`.
fn shutters(c: &mut Canvas, [x, y, z]: [i32; 3], out: Direction, half: &str) {
    let step = out.normal();
    for side in [-1, 1] {
        let (x, z) = (x + step.x + side * step.z, z + step.z + side * step.x);
        c.place(&trapdoor(out, half), x, y, z);
    }
}

fn shuttered_window(c: &mut Canvas, at: [i32; 3], out: Direction, half: &str) {
    c.place(&S.pane, at[0], at[1], at[2]);
    shutters(c, at, out, half);
}

/// Trapdoors hinged at `half` standing round the rectangle from `min` to
/// `max` of the layer `y`.
fn trapdoor_ring(c: &mut Canvas, y: i32, min: [i32; 2], max: [i32; 2], half: &str) {
    let ([x0, z0], [x1, z1]) = (min, max);
    c.solid(&trapdoor(North, half), [x0, y, z0 - 1], [x1, y, z0 - 1]);
    c.solid(&trapdoor(South, half), [x0, y, z1 + 1], [x1, y, z1 + 1]);
    c.solid(&trapdoor(West, half), [x0 - 1, y, z0], [x0 - 1, y, z1]);
    c.solid(&trapdoor(East, half), [x1 + 1, y, z0], [x1 + 1, y, z1]);
}

/// A flower pot: a jigsaw nothing joins, ringed by trapdoors.
fn planter(c: &mut Canvas, [x, y, z]: [i32; 3]) {
    trapdoor_ring(c, y, [x, z], [x, z], "bottom");
    c.socket_facing([x, y, z], "up_west", EMPTY, EMPTY, NOTHING);
}

/// A ring of stairs round `[x, z]` rising towards it, the corners facing as
/// `corners` says: north-west, north-east, south-west, south-east.
fn stair_ring(
    c: &mut Canvas,
    of: impl Fn(Direction) -> Cell,
    [x, y, z]: [i32; 3],
    corners: [Direction; 4],
) {
    let edges = [(South, 0, -1), (North, 0, 1), (East, -1, 0), (West, 1, 0)];
    for (facing, dx, dz) in edges {
        c.place(&of(facing), x + dx, y, z + dz);
    }
    for (facing, (dx, dz)) in corners
        .into_iter()
        .zip([(-1, -1), (1, -1), (-1, 1), (1, 1)])
    {
        c.place(&of(facing), x + dx, y, z + dz);
    }
}

/// The wall posts either side of the entrance at `[3, 0, 0]`, `height` high
/// with a torch on each. The lowest are `living`, or joined to the entrance
/// in the abandoned village.
fn gate_posts(c: &mut Canvas, v: Village, height: i32, living: &Cell) {
    let lowest = if v.zombie { &S.cobble_wall } else { living };
    for x in [2, 4] {
        c.torch_post(&S.cobble_wall, [x, 0, 0], height);
        c.place(lowest, x, 0, 0);
    }
}

/// A 5x5 room under a log roof: a door on the north, a shuttered window in
/// each other wall.
fn cottage(c: &mut Canvas, v: Village, post: &Cell, floor: &str) {
    c.walls(&S.cobble, post, [1, 0, 1], [5, 3, 5]);
    c.solid(&block(floor), [2, 0, 2], [4, 0, 4]);
    gabled_roof(c, Gable::new(Z, [0, 6], [0, 6], 3, 4), &S.cobble, [1, 5]);
    c.door(SPRUCE_DOOR, [3, 1, 1], South, Hinge::Left);
    shuttered_window(c, [1, 2, 3], West, "bottom");
    shuttered_window(c, [5, 2, 3], East, "bottom");
    shuttered_window(c, [3, 2, 5], South, "bottom");
    c.entrance([3, 0, 0], EMPTY, STEP_SOUTH);
    villagers(c, v, floor, &[[3, 0, 3]]);
}

fn small_house_1(c: &mut Canvas, v: Village) {
    c.void([0, 0, 0], [6, 0, 8]);
    c.void([0, 1, 3], [0, 1, 3]);
    c.solid(&GROUND, [1, 0, 1], [5, 0, 6]);
    c.fill(&GROUND, [1, 5], 0, 7);
    c.place(&S.path, 3, 0, 0);
    c.walls(&S.cobble, &K.log, [1, 1, 2], [5, 4, 6]);
    c.posts(&K.log, &[1, 5], &[1, 7], [1, 4]);
    c.solid(&S.spruce_planks, [2, 1, 3], [4, 1, 5]);
    c.fill(&K.log_x, 2..=4, 5, [1, 2, 6, 7]);
    c.fill(&S.cobble, 3, 6, [2, 6]);
    log_roof(c, Gable::new(Z, [0, 6], [0, 8], 4, 4));
    c.fill(&S.cobble_wall, [2, 4], 1, 1);
    c.fill(&S.torch, [2, 4], 2, 1);
    c.place(&stairs(COBBLE_STAIRS, South), 3, 1, 1);
    c.door(SPRUCE_DOOR, [3, 2, 2], South, Hinge::Right);
    shuttered_window(c, [1, 3, 4], West, "top");
    shuttered_window(c, [5, 3, 4], East, "top");
    shuttered_window(c, [3, 3, 6], South, "top");
    c.bed(PURPLE_BED, [2, 2, 4], South);
    c.place(&stairs(SPRUCE_STAIRS, East), 4, 2, 4);
    for (z, facing) in [(3, "north"), (5, "south")] {
        let sign = format!("minecraft:spruce_wall_sign[facing={facing},waterlogged=false]");
        c.machine([4, 2, z], &sign, SIGN_DATA);
    }
    c.place(&S.wall_torch, 3, 5, 3);
    c.entrance([3, 1, 0], EMPTY, NOTHING);
    villagers(c, v, PLANKS, &[[2, 1, 3]]);
}

fn small_house_2(c: &mut Canvas, v: Village) {
    cottage(c, v, &S.cobble, PLANKS);
    c.posts(&K.log, &[2, 4], &[2, 4], [0, 0]);
    gate_posts(c, v, 1, &wall_post(&[South], &[]));
    c.place(&K.fern, 1, 0, 0);
    c.place(&K.large_fern, 5, 0, 0);
    c.fill(&stairs(SPRUCE_STAIRS, West), 2, 1, [2, 4]);
    c.place(&S.spruce_planks, 2, 1, 3);
    c.place(&S.torch, 2, 2, 3);
    c.bed(PURPLE_BED, [4, 1, 3], South);
}

fn small_house_3(c: &mut Canvas, v: Village) {
    cottage(c, v, &K.log, COBBLE);
    gate_posts(c, v, 2, &wall_post(&[], &[South]));
    c.place(&S.poppy, 6, 0, 0);
    chest(c, [2, 1, 2], East, HOUSE_LOOT);
    c.place(&S.crafting_table, 4, 1, 2);
    c.bed(BLUE_BED, [2, 1, 3], South);
    c.fill(&stairs(SPRUCE_STAIRS, East), 4, 1, [3, 4]);
    c.fill(&S.wall_torch, 3, 4, [2, 4]);
}

fn small_house_4(c: &mut Canvas, v: Village) {
    c.void([0, 0, 0], [6, 0, 7]);
    c.place(&S.air, 0, 0, 0);
    c.void([2, 1, 0], [2, 1, 0]);
    c.boxes(&S.dirt, &[([1, 0, 1], [5, 0, 4]), ([2, 0, 5], [4, 0, 6])]);
    c.solid(&S.cobble, [2, 0, 2], [4, 0, 3]);
    c.place(&S.cobble, 3, 0, 4);
    c.solid(&S.spruce_planks, [3, 0, 5], [3, 0, 6]);
    c.place(&S.path, 3, 0, 7);

    c.walls(&S.cobble, &S.cobble, [1, 1, 1], [5, 3, 4]);
    gabled_roof(c, Gable::new(X, [0, 5], [0, 6], 3, 3), &S.cobble, [1, 5]);
    c.fill(&S.spruce_planks, [2, 4], 1..=2, [5, 6]);
    c.solid(&S.spruce_planks, [2, 3, 5], [4, 3, 6]);
    c.fill(&S.spruce_planks, 3, 4, [5, 6]);
    log_roof(c, Gable::new(Z, [1, 5], [5, 7], 3, 3));
    c.solid(&K.log_z, [3, 4, 4], [3, 5, 4]);

    for z in [4, 6] {
        c.door(SPRUCE_DOOR, [3, 1, z], North, Hinge::Right);
    }
    for x in [2, 4] {
        shuttered_window(c, [x, 2, 1], North, "top");
    }
    c.place(&S.crafting_table, 2, 1, 2);
    c.bed(BLUE_BED, [4, 1, 3], North);
    c.fill(&S.wall_torch, 3, 4, [2, 3]);
    c.place(&S.wall_torch, 3, 3, 7);
    c.entrance([3, 1, 7], EMPTY, NOTHING);
    decorations(c, v, GRASS, &[[0, 6]]);
    villagers(c, v, COBBLE, &[[2, 0, 3]]);
}

fn small_house_5(c: &mut Canvas, v: Village) {
    c.walls(&S.cobble, &S.cobble, [2, 0, 1], [6, 3, 5]);
    c.solid(&S.spruce_planks, [3, 0, 2], [5, 0, 4]);
    c.posts(&K.log, &[1, 7], &[1, 5], [0, 3]);
    gabled_roof(c, Gable::new(X, [0, 6], [0, 8], 3, 4), &S.cobble, [2, 6]);
    c.door(SPRUCE_DOOR, [2, 1, 3], East, Hinge::Left);
    shuttered_window(c, [4, 2, 1], North, "top");
    shuttered_window(c, [4, 2, 5], South, "top");
    shuttered_window(c, [6, 2, 3], East, "top");
    c.fill(&K.large_fern, 1, 0, [2, 4]);
    chest(c, [3, 1, 2], South, HOUSE_LOOT);
    c.bed(PURPLE_BED, [4, 1, 2], East);
    c.place(&stairs(COBBLE_STAIRS, East), 5, 1, 4);
    c.fill(&S.wall_torch, [1, 3, 5], 3, 3);
    west_entrance(c, [1, 0, 3], STEP_EAST);
    villagers(c, v, PLANKS, &[[4, 0, 3]]);
}

fn medium_house_1(c: &mut Canvas, v: Village) {
    let walk = [7, 0, 7, 1, 6, 2, 7, 2, 6, 3, 6, 4, 6, 5, 6, 6];
    let walk = walk.as_chunks().0;
    c.void([0, 2, 0], [7, 2, 6]);
    c.solid(&GROUND, [0, 0, 0], [7, 1, 6]);
    c.boxes(&GROUND, &[([2, 2, 0], [5, 2, 0]), ([2, 2, 1], [4, 2, 1])]);
    c.fill(&GROUND, [2, 4], 2, 2);
    c.fill(&GROUND, [1, 5], 2, [4, 5]);
    c.fill(&GROUND, 7, 2, [3, 5]);
    c.scatter(&v.underfoot(), 1, walk);
    c.scatter(&S.path, 2, walk);

    c.solid(&S.air, [2, 0, 0], [4, 1, 2]);
    c.solid(&S.air, [3, 1, 2], [3, 2, 3]);
    c.solid(&S.air, [2, 1, 5], [4, 2, 6]);
    c.fill(&S.cobble, [2, 4], 1..=2, [3, 4]);
    c.fill(&S.cobble, [2, 4], 0, 3);
    c.place(&S.cobble, 3, 0, 4);
    c.place(&stairs(COBBLE_STAIRS, South), 3, 0, 3);
    c.walls(&S.cobble, &S.cobble, [2, 3, 2], [4, 4, 4]);
    c.door(SPRUCE_DOOR, [3, 1, 4], North, Hinge::Left);
    for [x, y, z] in [[4, 1, 6], [5, 2, 6]] {
        c.place(&v.underfoot(), x, y - 1, z);
        c.place(&stairs(COBBLE_STAIRS, East), x, y, z);
    }
    c.fill(&stairs(SPRUCE_STAIRS, West), 2, 0, [0, 1]);
    chest(c, [2, 0, 2], East, HOUSE_LOOT);
    c.bed(PURPLE_BED, [4, 0, 1], North);
    c.place(&wall_torch(South), 3, 1, 0);

    c.place(&v.underfoot(), 6, 2, 0);
    for step in 0..3 {
        c.place(&stairs(SPRUCE_STAIRS, West), 6 - step, 3 + step, 0);
    }
    c.each(&S.spruce_slab_top, &[[5, 3, 0], [4, 4, 0]]);
    c.solid(&S.spruce_fence, [3, 3, 0], [3, 4, 0]);
    c.place(&S.spruce_planks, 3, 5, 0);

    c.solid(&S.spruce_planks, [2, 5, 2], [4, 5, 4]);
    c.solid(&top_stairs(SPRUCE_STAIRS, South), [2, 5, 1], [5, 5, 1]);
    c.solid(&top_stairs(SPRUCE_STAIRS, West), [5, 5, 2], [5, 5, 5]);
    c.solid(&top_stairs(SPRUCE_STAIRS, North), [1, 5, 5], [4, 5, 5]);
    c.solid(&top_stairs(SPRUCE_STAIRS, East), [1, 5, 1], [1, 5, 4]);
    c.walls(&S.spruce_planks, &K.log, [1, 6, 1], [5, 7, 5]);
    gabled_roof(
        c,
        Gable::new(Z, [0, 6], [0, 6], 7, 4),
        &S.spruce_planks,
        [1, 5],
    );
    c.door(SPRUCE_DOOR, [3, 6, 1], South, Hinge::Right);
    shuttered_window(c, [3, 7, 5], South, "top");
    c.bed(BLUE_BED, [2, 6, 3], South);
    c.place(&stairs(SPRUCE_STAIRS, North), 4, 6, 2);
    c.place(&S.spruce_fence, 4, 6, 3);
    c.place(&K.plate, 4, 7, 3);
    c.place(&stairs(SPRUCE_STAIRS, South), 4, 6, 4);
    c.fill(&S.wall_torch, 3, 8, [0, 4]);
    c.place(&S.wall_torch, 3, 3, 5);

    c.fill(&K.large_fern, [1, 5], 3, 4);
    c.place(&K.large_fern, 1, 3, 5);
    c.fill(&K.fern, [5, 7], 3, 5);
    c.entrance([5, 3, 6], EMPTY, v.doorway());
    villagers(c, v, PLANKS, &[[3, 5, 3]]);
    if v.zombie {
        c.boxes(&VOID, &[([2, 1, 5], [4, 2, 5]), ([2, 2, 6], [4, 2, 6])]);
        c.void([2, 4, 5], [4, 4, 5]);
        c.each(&VOID, &[[3, 1, 6], [2, 3, 5], [4, 3, 5], [3, 3, 6]]);
        c.place(&GROUND, 2, 1, 6);
    } else {
        c.place(&wall_torch(North), 3, 3, 3);
        c.fill(&S.wall_torch, [1, 5], 4, 3);
        villagers(c, v, PLANKS, &[[2, 5, 2]]);
    }
}

fn medium_house_2(c: &mut Canvas, v: Village) {
    c.void([0, 0, 0], [6, 0, 7]);
    c.boxes(&GROUND, &[([4, 0, 0], [5, 0, 0]), ([2, 0, 1], [5, 0, 2])]);
    c.solid(&GROUND, [1, 0, 3], [5, 0, 6]);
    c.place(&GROUND, 6, 0, 2);
    c.fill(&S.path, [3, 6], 0, 0);
    c.solid(&S.spruce_planks, [3, 0, 1], [3, 0, 2]);
    c.solid(&S.cobble, [2, 0, 4], [4, 0, 5]);
    c.place(&S.cobble, 3, 0, 3);

    c.walls(&S.cobble, &S.cobble, [1, 1, 3], [5, 8, 6]);
    c.solid(&S.spruce_planks, [2, 4, 4], [4, 4, 5]);
    gabled_roof(c, Gable::new(X, [2, 7], [0, 6], 8, 3), &S.cobble, [1, 5]);
    c.fill(&S.spruce_planks, [2, 4], 1..=2, 1..=2);
    c.place(&S.spruce_planks, 3, 3, 1);
    log_roof(c, Gable::new(Z, [1, 5], [0, 2], 2, 3));
    for z in [1, 3] {
        c.door(SPRUCE_DOOR, [3, 1, z], South, Hinge::Right);
    }

    c.place(&v.underfoot(), 6, 0, 1);
    for step in 0..4 {
        c.place(&stairs(SPRUCE_STAIRS, South), 6, 1 + step, 1 + step);
    }
    for step in 0..3 {
        c.place(&top_stairs(SPRUCE_STAIRS, North), 6, 1 + step, 2 + step);
    }
    c.place(&S.spruce_planks, 6, 4, 5);
    c.door(SPRUCE_DOOR, [5, 5, 5], West, Hinge::Right);

    for x in [2, 4] {
        shuttered_window(c, [x, 2, 6], South, "top");
        shuttered_window(c, [x, 6, 6], South, "top");
        shuttered_window(c, [x, 6, 3], North, "top");
    }
    c.place(&S.crafting_table, 2, 1, 5);
    c.bed(BLUE_BED, [3, 1, 5], East);
    c.bed(PURPLE_BED, [3, 5, 4], West);
    chest(c, [4, 5, 4], South, HOUSE_LOOT);
    c.fill(&wall_torch(North), 3, 3, [0, 2]);
    c.place(&S.wall_torch, 3, 3, 4);
    c.place(&S.wall_torch, 6, 6, 4);
    if v.lit() {
        c.fill(&wall_torch(East), 2, 9, [4, 5]);
        c.fill(&wall_torch(West), 4, 9, [4, 5]);
    }
    c.entrance([4, 1, 0], EMPTY, NOTHING);
    villagers(c, v, COBBLE, &[[2, 0, 4]]);
    villagers(c, v, PLANKS, &[[3, 4, 5]]);
}

fn medium_house_3(c: &mut Canvas, v: Village) {
    c.void([0, 0, 0], [7, 0, 12]);
    c.void([1, 1, 11], [1, 1, 11]);
    c.each(
        &S.air,
        &[[1, 0, 1], [7, 0, 4], [7, 0, 7], [0, 0, 10], [6, 0, 12]],
    );
    c.place(&GROUND, 6, 0, 0);
    c.place(&K.large_fern, 6, 1, 0);
    c.solid(&S.dirt, [3, 0, 1], [6, 0, 11]);
    c.boxes(&S.cobble, &[([3, 0, 2], [5, 0, 4]), ([4, 0, 5], [5, 0, 7])]);
    c.solid(&S.cobble, [3, 0, 8], [5, 0, 10]);
    c.place(&S.dirt, 0, 0, 6);

    c.solid(&S.cobble, [6, 1, 1], [6, 3, 11]);
    c.fill(&S.cobble, 3..=5, 1..=3, [1, 11]);
    c.solid(&S.cobble, [3, 1, 5], [3, 3, 7]);
    gabled_roof(c, Gable::new(Z, [2, 7], [0, 12], 3, 3), &S.cobble, [1, 11]);
    c.place(&S.air, 2, 3, 6);
    shuttered_window(c, [3, 2, 6], West, "top");
    for z in [3, 6, 9] {
        shuttered_window(c, [6, 2, z], East, "top");
    }
    c.fill(&S.bookshelf, [4, 5], 4, [2, 10]);

    for (z, hinge) in [(3, Hinge::Left), (9, Hinge::Right)] {
        c.fill(&S.dirt, 1..=2, 0, [z - 1, z + 1]);
        c.solid(&S.cobble, [1, 0, z], [2, 0, z]);
        c.place(&S.path, 0, 0, z);
        c.fill(&S.spruce_planks, 1..=2, 1..=3, [z - 1, z + 1]);
        log_roof(c, Gable::new(X, [z - 2, z + 2], [0, 2], 3, 3));
        c.place(&K.log_x, 3, 5, z);
        c.solid(&S.spruce_planks, [1, 4, z], [2, 4, z]);
        c.place(&S.spruce_planks, 1, 3, z);
        c.door(SPRUCE_DOOR, [1, 1, z], East, hinge);
        c.place(&wall_torch(West), 0, 3, z);
        c.place(&wall_torch(East), 2, 3, z);
    }

    chest(c, [3, 1, 2], South, HOUSE_LOOT);
    c.place(&S.crafting_table, 3, 1, 10);
    for z in [2, 10] {
        c.bed(BLUE_BED, [4, 1, z], East);
    }
    c.fill(&S.spruce_fence, 5, 1, [3, 6, 9]);
    c.place(&shut_trapdoor(South, "bottom"), 5, 2, 3);
    c.fill(&shut_trapdoor(West, "bottom"), 5, 2, [6, 9]);
    c.place(&stairs(SPRUCE_STAIRS, North), 5, 1, 5);
    c.place(&stairs(SPRUCE_STAIRS, South), 5, 1, 7);
    c.place(&S.wall_torch, 4, 3, 6);

    west_entrance(c, [0, 1, 6], v.doorway());
    c.spot([2, 0, 0], CATS, GRASS);
    decorations(c, v, GRASS, &[[1, 6]]);
    if !v.zombie {
        villagers(c, v, COBBLE, &[[4, 0, 4], [4, 0, 8]]);
    }
}

fn medium_house_4(c: &mut Canvas, v: Village) {
    c.solid(&S.dirt, [1, 0, 1], [7, 0, 7]);
    c.solid(&S.spruce_planks, [2, 0, 2], [6, 0, 6]);
    c.place(&S.dirt, 4, 0, 4);
    c.fill(&S.cobble, 4, 0, [5, 7]);
    c.place(&S.path, 4, 0, 8);

    c.walls(&S.cobble, &K.log, [1, 1, 1], [7, 2, 7]);
    for z in [1, 7] {
        c.solid(&S.cobble, [3, 3, z], [5, 3, z]);
        c.fill(&K.log_x, [2, 6], 3, z);
        c.solid(&K.log_x, [3, 4, z], [5, 4, z]);
    }
    if v.zombie {
        c.fill(&K.log, [2, 6], 3, 7);
    }
    log_roof(c, Gable::new(Z, [0, 8], [0, 8], 2, 4));
    c.fill(&K.log_z, 4, 5, [0..=2, 6..=8]);

    c.furnace([4, 1, 4], "furnace", South);
    c.solid(&S.cobble, [4, 2, 4], [4, 3, 4]);
    let collar = |facing| top_stairs(COBBLE_STAIRS, facing);
    stair_ring(c, collar, [4, 4, 4], [South, West, North, West]);
    campfire(c, [4, 4, 4], North, false);
    c.walls(&S.cobble, &S.cobble, [3, 5, 3], [5, 5, 5]);
    let cap = |facing| stairs(COBBLE_STAIRS, facing);
    stair_ring(c, cap, [4, 6, 4], [East, South, North, North]);

    c.door(SPRUCE_DOOR, [4, 1, 7], North, Hinge::Right);
    for x in [3, 5] {
        shuttered_window(c, [x, 2, 1], North, "top");
    }
    c.bed(PURPLE_BED, [3, 1, 2], West);
    c.place(&S.spruce_planks, 4, 1, 2);
    c.bed(PURPLE_BED, [5, 1, 2], East);
    c.place(&trapdoor(North, "bottom"), 4, 1, 3);
    c.place(&trapdoor(West, "bottom"), 3, 1, 4);
    c.place(&trapdoor(East, "bottom"), 5, 1, 4);
    c.fill(&stairs(SPRUCE_STAIRS, West), 2, 1, [5, 6]);
    c.fill(&stairs(SPRUCE_STAIRS, East), 6, 1, [5, 6]);
    c.fill(&S.wall_torch, [3, 5], 2, 4);
    c.place(&S.wall_torch, 4, 2, 5);
    c.place(&S.wall_torch, 4, 4, 8);
    c.entrance([5, 1, 8], EMPTY, v.doorway());
    if !v.zombie {
        c.place(&S.wall_torch, 4, 4, 0);
        villagers(c, v, PLANKS, &[[6, 0, 4], [3, 0, 6]]);
    }
}

fn taiga_animal_pen_1(c: &mut Canvas) {
    trapdoor_ring(c, 0, [1, 1], [11, 6], "bottom");
    c.solid(&GROUND, [1, 0, 1], [11, 0, 6]);
    c.fence_ring(&S.spruce_fence, &K.gate, [1, 1, 1], [11, 6], &[[6, 6]]);
    c.posts(&S.spruce_fence, &[1, 11], &[1, 6], [2, 2]);
    c.posts(&S.spruce_fence, &[5, 7], &[6], [2, 3]);
    c.place(&S.spruce_planks, 6, 3, 6);
    c.place(&S.torch, 6, 4, 6);
    c.place(&trapdoor(North, "bottom"), 6, 3, 5);
    c.place(&trapdoor(South, "bottom"), 6, 3, 7);
    trapdoor_ring(c, 1, [3, 3], [4, 3], "bottom");
    c.entrance([6, 0, 7], EMPTY, "minecraft:spruce_stairs[facing=north]");
    spots(c, ANIMALS, GRASS, &[[6, 0, 4], [8, 0, 3]]);
}

fn taiga_armorer_2(c: &mut Canvas) {
    let path = [
        2, 2, 0, 1, 1, 1, 3, 3, 1, 5, 5, 1, 0, 1, 2, 0, 1, 3, 6, 6, 3, 0, 1, 4, 2, 3, 5, 5, 5, 5,
        0, 0, 6, 2, 2, 6, 6, 6, 6,
    ];
    let grass = [
        3, 4, 0, 2, 2, 1, 4, 4, 1, 3, 6, 2, 5, 5, 3, 3, 5, 4, 1, 1, 5, 4, 4, 5, 6, 6, 5, 3, 3, 6,
    ];
    c.void([0, 0, 0], [6, 0, 6]);
    c.runs(&S.path, 0, path.as_chunks().0);
    c.runs(&S.grass, 0, grass.as_chunks().0);
    c.solid(&S.cobble, [2, 0, 2], [2, 0, 4]);
    c.place(&S.cobble, 3, 0, 3);
    c.place(&S.dirt, 4, 0, 3);

    let hearth = |facing| stairs(COBBLE_STAIRS, facing);
    stair_ring(c, hearth, [4, 1, 3], [East, West, East, West]);
    c.place(&S.air, 3, 1, 3);
    c.furnace([4, 1, 3], "blast_furnace", West);
    c.solid(&S.cobble_wall, [4, 2, 3], [4, 7, 3]);
    c.posts(&S.cobble_wall, &[3], &[0, 6], [1, 3]);
    c.solid(&K.log_z, [3, 4, 0], [3, 4, 6]);
    log_roof(c, Gable::new(Z, [2, 4], [1, 5], 4, 2));
    c.place(&S.cobble, 4, 4, 3);
    c.place(&S.wall_torch, 1, 4, 3);

    campfire(c, [1, 1, 5], North, false);
    c.scatter(&K.large_fern, 1, &[[4, 0], [6, 2]]);
    c.place(&K.fern, 6, 1, 5);
    c.entity(
        [2.5, 1.0, 1.5],
        [2, 1, 1],
        &[ARMOR_STAND, ARMOR_STAND_ENTITY],
    );
    c.entity(
        [4.5, 1.0, 5.5],
        [4, 1, 5],
        &[ARMOR_STAND, ARMOR_STAND_ENTITY_2],
    );
    c.entrance([0, 1, 3], EMPTY, NOTHING);
    decorations(c, LIVING, GRASS, &[[0, 0]]);
    c.spot([6, 0, 4], CATS, GRASS);
}

fn taiga_armorer_house_1(c: &mut Canvas) {
    c.void([0, 0, 0], [9, 0, 6]);
    c.fill(&GROUND, 0, 0, [1, 6]);
    c.fill(&S.poppy, 0, 1, [1, 6]);
    c.solid(&S.dirt, [1, 0, 1], [8, 0, 5]);
    c.solid(&S.spruce_planks, [2, 0, 2], [7, 0, 2]);
    c.fill(&S.spruce_planks, [2, 7], 0, 3..=4);
    c.solid(&S.cobble, [3, 0, 3], [6, 0, 3]);
    c.place(&S.cobble, 1, 0, 3);
    c.place(&S.path, 0, 0, 3);

    c.walls(&S.cobble, &S.cobble, [1, 1, 1], [8, 2, 5]);
    gabled_roof(c, Gable::new(X, [0, 6], [0, 9], 2, 4), &S.cobble, [1, 8]);
    c.door(SPRUCE_DOOR, [1, 1, 3], East, Hinge::Right);
    shuttered_window(c, [8, 2, 3], East, "top");

    c.fill(&S.cobble, [3, 6], 1, 4);
    for x in [4, 5] {
        c.furnace([x, 1, 4], "blast_furnace", North);
        c.solid(&S.cobble, [x, 2, 4], [x, 3, 4]);
    }
    c.place(&stairs(COBBLE_STAIRS, East), 3, 2, 4);
    c.place(&stairs(COBBLE_STAIRS, West), 6, 2, 4);
    c.place(&S.cobble, 5, 4, 4);
    c.solid(&S.cobble_wall, [5, 5, 4], [5, 6, 4]);
    c.fill(&S.wall_torch, [0, 2, 7], 3, 3);
    c.entrance([0, 1, 3], EMPTY, NOTHING);
}

fn taiga_butcher_shop_1(c: &mut Canvas) {
    c.boxes(&S.cobble, &[([1, 0, 1], [9, 0, 4]), ([1, 0, 5], [5, 0, 6])]);
    c.solid(&S.cobble, [2, 0, 7], [4, 0, 7]);
    c.boxes(&K.log, &[([2, 0, 3], [4, 0, 5]), ([3, 0, 2], [4, 0, 2])]);
    c.solid(&smooth_slab("top"), [5, 0, 2], [8, 0, 3]);
    c.solid(&smooth_slab("double"), [8, 1, 2], [8, 1, 3]);
    c.place(&smooth_slab("double"), 7, 0, 2);
    c.solid(&stairs(COBBLE_STAIRS, North), [2, 0, 8], [3, 0, 8]);

    c.solid(&S.grass, [6, 0, 5], [9, 0, 7]);
    c.solid(&trapdoor(East, "bottom"), [10, 0, 5], [10, 0, 7]);
    c.solid(&trapdoor(South, "bottom"), [6, 0, 8], [9, 0, 8]);
    c.place(&trapdoor(West, "bottom"), 5, 0, 7);
    c.solid(&S.spruce_fence, [6, 1, 7], [9, 1, 7]);
    c.solid(&S.spruce_fence, [9, 1, 5], [9, 1, 6]);
    c.place(&S.spruce_fence, 6, 1, 6);
    c.place(&S.torch, 9, 2, 7);

    c.walls(&S.cobble, &S.cobble, [1, 1, 1], [9, 3, 4]);
    c.solid(&S.cobble, [5, 4, 2], [9, 4, 3]);
    c.solid(&S.cobble, [1, 4, 2], [1, 4, 3]);
    log_roof(c, Gable::new(X, [0, 5], [0, 10], 3, 3));

    c.walls(&S.cobble, &S.cobble, [1, 1, 4], [5, 3, 6]);
    c.solid(&S.air, [2, 1, 4], [4, 3, 5]);
    gable_end(c, &S.cobble, Gable::new(Z, [0, 6], [4, 7], 3, 4), 6);
    c.solid(&K.log_z, [2, 4, 2], [4, 4, 5]);
    c.fill(&K.log_z, 6, 3, 5..=7);
    c.fill(&K.log_z, 0, 3, 6..=7);
    log_roof(c, Gable::new(Z, [1, 5], [4, 7], 4, 3));
    c.place(&K.log, 3, 5, 4);
    c.place(&K.log_z, 3, 5, 5);

    c.furnace([5, 1, 2], "smoker", South);
    c.place(&wall_post(&[], &[North]), 5, 2, 2);
    let pillar = "minecraft:cobblestone_wall[east=tall,north=tall,south=tall,up=false,waterlogged=false,west=tall]";
    c.place(&block(pillar), 5, 3, 2);
    c.place(&trapdoor(West, "top"), 4, 3, 2);
    c.place(&trapdoor(East, "top"), 6, 3, 2);
    c.place(&trapdoor(South, "top"), 5, 3, 3);
    campfire(c, [5, 4, 2], North, false);
    c.place(&top_stairs(COBBLE_STAIRS, East), 4, 5, 1);
    c.place(&top_stairs(COBBLE_STAIRS, West), 6, 5, 1);
    c.each(&S.cobble, &[[5, 5, 1], [4, 5, 2], [5, 5, 3], [6, 5, 3]]);
    c.place(&S.air, 5, 5, 2);
    let cap = |facing| stairs(COBBLE_STAIRS, facing);
    stair_ring(c, cap, [5, 6, 2], [East, South, North, West]);

    c.door(SPRUCE_DOOR, [7, 1, 4], North, Hinge::Right);
    c.door(SPRUCE_DOOR, [3, 1, 6], North, Hinge::Left);
    for x in [3, 7] {
        shuttered_window(c, [x, 2, 1], North, "top");
    }
    c.solid(&S.pane, [1, 2, 2], [1, 2, 3]);
    c.fill(&trapdoor(West, "top"), 0, 2, [1, 4]);
    c.place(&trapdoor(East, "bottom"), 3, 1, 2);
    c.place(&trapdoor(South, "bottom"), 2, 1, 3);
    c.place(&S.wall_torch, 3, 3, 2);
    c.fill(&wall_torch(West), 8, 3, 2..=3);
    c.place(&S.wall_torch, 3, 3, 5);
    c.place(&S.wall_torch, 3, 4, 7);

    c.scatter(&K.fern, 0, &[[3, 0], [8, 0], [10, 3]]);
    c.scatter(&S.poppy, 0, &[[4, 0], [5, 0], [0, 2], [1, 8]]);
    let ferns = [[10, 2], [10, 4], [0, 5], [1, 7], [5, 8]];
    c.scatter(&K.large_fern, 0, &ferns);
    c.entrance(
        [4, 0, 8],
        EMPTY,
        "minecraft:cobblestone_stairs[facing=north]",
    );
    c.spot([7, 0, 6], "minecraft:village/common/butcher_animals", GRASS);
}

fn cartographer_house_1(c: &mut Canvas, v: Village) {
    c.fill(&GROUND, [2, 4], 0, 0..=1);
    c.solid(&S.path, [3, 0, 0], [3, 0, 1]);
    c.solid(&S.dirt, [1, 0, 2], [5, 0, 6]);
    c.solid(&S.cobble, [2, 0, 3], [4, 0, 5]);
    c.place(&S.cobble, 3, 0, 2);
    for x in [2, 4] {
        c.torch_post(&wall_post(&[South], &[]), [x, 1, 0], 1);
    }
    c.posts(&K.log, &[2, 4], &[1], [1, 7]);

    c.walls(&S.cobble, &S.cobble, [1, 1, 2], [5, 2, 6]);
    c.solid(&S.cobble, [1, 3, 2], [5, 3, 6]);
    c.door(SPRUCE_DOOR, [3, 1, 2], South, Hinge::Right);
    shuttered_window(c, [1, 2, 4], West, "top");
    shuttered_window(c, [5, 2, 4], East, "top");
    shuttered_window(c, [3, 2, 6], South, "top");
    c.place(&block("minecraft:cartography_table"), 4, 1, 5);
    let ladder = block("minecraft:ladder[facing=north,waterlogged=false]");
    c.solid(&ladder, [2, 2, 5], [2, 3, 5]);
    c.place(&wall_torch(East), 2, 2, 3);
    c.place(&wall_torch(West), 4, 2, 3);

    c.place(&shut_trapdoor(North, "top"), 3, 3, 1);
    c.place(&S.grass, 3, 4, 1);
    c.place(&S.poppy, 3, 5, 1);
    c.place(&trapdoor(North, "bottom"), 3, 4, 0);

    c.walls(&S.spruce_planks, &S.spruce_planks, [1, 4, 2], [5, 6, 6]);
    gabled_roof(
        c,
        Gable::new(Z, [0, 6], [0, 7], 6, 4),
        &S.spruce_planks,
        [2, 6],
    );
    c.posts(&K.log, &[2, 4], &[2, 6], [4, 7]);
    c.posts(&K.log, &[1, 5], &[3, 5], [4, 6]);
    c.fill(&S.pane, 3, [5, 7], 2);
    c.place(&S.pane, 3, 5, 6);
    c.fill(&S.pane, [1, 5], 5, 4);
    chest(c, [2, 4, 3], South, "village_cartographer");
    c.fill(&stairs(SPRUCE_STAIRS, East), 4, 4, [3, 5]);
    c.place(&S.spruce_fence, 4, 4, 4);
    c.place(&shut_trapdoor(West, "bottom"), 4, 5, 4);
    c.fill(&S.wall_torch, 3, 6, [3, 5]);
    c.place(&wall_torch(North), 3, 8, 1);
    c.entrance([3, 1, 0], EMPTY, NOTHING);
    decorations(c, v, GRASS, &[[0, 0]]);
}

fn fisher_cottage_1(c: &mut Canvas, v: Village) {
    let shore = [
        0, 0, 1, 0, 0, 4, 7, 8, 4, 8, 8, 5, 7, 8, 6, 0, 0, 7, 1, 1, 8, 7, 9, 8, 0, 0, 9, 8, 9, 9,
        0, 0, 10, 4, 5, 10, 7, 9, 10, 0, 4, 11, 7, 8, 11,
    ];
    let gravel = [6, 8, 0, 3, 9, 1, 4, 8, 2, 4, 8, 3, 6, 6, 4];
    let sand = [4, 0, 5, 0, 9, 2, 9, 3, 9, 4, 7, 5, 9, 5, 9, 6, 8, 7, 9, 7];
    let grass = [
        1, 9, 0, 1, 1, 1, 9, 9, 1, 0, 0, 2, 8, 9, 2, 0, 0, 3, 9, 9, 3, 1, 1, 4, 9, 9, 5, 0, 0, 6,
        9, 9, 6, 7, 9, 7, 0, 0, 8, 1, 1, 10, 3, 3, 10, 6, 6, 11,
    ];
    let water = [
        2, 6, 1, 1, 3, 2, 5, 7, 2, 1, 3, 3, 5, 8, 3, 3, 3, 4, 5, 5, 4, 7, 8, 4, 8, 8, 5, 7, 8, 6,
    ];
    let dirt = [
        7, 8, 1, 9, 9, 4, 1, 2, 5, 2, 2, 6, 6, 6, 6, 2, 2, 7, 6, 6, 7, 2, 2, 8, 6, 6, 8, 1, 3, 9,
        5, 7, 9, 2, 2, 10, 6, 6, 10,
    ];
    c.solid(&S.dirt, [0, 0, 0], [9, 0, 11]);
    c.runs(&S.grass, 0, shore.as_chunks().0);
    c.runs(&block("minecraft:gravel"), 0, gravel.as_chunks().0);
    c.scatter(&block("minecraft:sand"), 0, sand.as_chunks().0);
    c.scatter(
        &block("minecraft:clay"),
        0,
        &[[5, 5], [6, 5], [6, 6], [7, 7]],
    );
    c.jigsaw_layer(1);
    c.runs(&S.grass, 1, grass.as_chunks().0);
    c.runs(&S.water, 1, water.as_chunks().0);
    c.runs(&S.dirt, 1, dirt.as_chunks().0);
    c.solid(&S.air, [1, 1, 6], [1, 1, 7]);
    c.solid(&S.path, [4, 1, 10], [4, 1, 11]);
    c.solid(&S.spruce_planks, [4, 1, 2], [4, 1, 4]);

    c.solid(&S.cobble, [3, 1, 5], [6, 1, 5]);
    c.solid(&S.cobble, [3, 1, 6], [5, 1, 8]);
    c.solid(&S.spruce_planks, [3, 1, 7], [5, 1, 7]);
    c.place(&S.cobble, 4, 1, 9);
    c.walls(&S.cobble, &S.cobble, [2, 2, 5], [6, 4, 9]);
    c.posts(&K.log, &[1, 7], &[5, 9], [2, 3]);
    c.place(&K.log, 7, 1, 5);
    c.posts(&K.log, &[2, 6], &[4], [1, 4]);
    c.posts(&K.log, &[2, 6], &[10], [2, 4]);
    c.solid(&K.log_x, [3, 5, 4], [5, 5, 4]);
    gabled_roof(c, Gable::new(Z, [1, 7], [3, 11], 4, 4), &S.cobble, [5, 9]);
    c.place(&S.spruce_fence, 4, 5, 9);
    c.open_door("minecraft:spruce_door", [4, 2, 5], East, Hinge::Left);
    c.door(SPRUCE_DOOR, [4, 2, 9], North, Hinge::Right);

    c.place(&trapdoor(West, "bottom"), 3, 2, 4);
    c.place(&trapdoor(East, "bottom"), 5, 2, 4);
    c.solid(&trapdoor(West, "bottom"), [4, 2, 6], [4, 2, 8]);
    c.solid(&S.water, [5, 2, 6], [5, 2, 8]);
    c.place(&S.spruce_fence, 3, 2, 6);
    c.place(&shut_trapdoor(East, "bottom"), 3, 3, 6);
    c.barrel([8, 2, 1], Direction::Up);
    c.barrel([9, 2, 4], Direction::Up);
    c.fill(&wall_torch(North), [3, 5], 3, [4, 8]);
    c.fill(&wall_torch(South), [3, 5], 3, 10);
    c.place(&S.wall_torch, 4, 5, 6);
    c.scatter(&K.large_fern, 2, &[[1, 1], [0, 6], [1, 10], [6, 11]]);
    c.place(&K.fern, 0, 2, 8);
    c.place(&S.poppy, 3, 2, 10);
    c.entrance([4, 2, 11], EMPTY, NOTHING);
    spots(c, CATS, GRASS, &[[0, 1, 5], [5, 1, 11]]);
    spots(c, &v.pool("decor"), GRASS, &[[0, 1, 0], [9, 1, 11]]);
}

fn taiga_fletcher_house_1(c: &mut Canvas) {
    c.walls(&S.cobble, &S.cobble, [5, 0, 1], [8, 3, 9]);
    c.solid(&K.log, [6, 0, 2], [7, 0, 8]);
    c.solid(&S.spruce_planks, [6, 0, 4], [7, 0, 6]);
    c.fill(&K.log, 5, 0, [4, 6]);
    c.solid(&S.air, [5, 1, 4], [5, 3, 6]);
    gabled_roof(c, Gable::new(Z, [4, 9], [0, 10], 3, 3), &S.cobble, [1, 9]);

    c.fill(&S.cobble, 3..=4, 0..=3, [4, 6]);
    c.solid(&S.cobble, [3, 3, 5], [3, 3, 5]);
    c.solid(&S.cobble, [3, 4, 5], [4, 4, 5]);
    c.place(&S.cobble, 3, 0, 5);
    c.solid(&S.spruce_planks, [4, 0, 5], [5, 0, 5]);
    log_roof(c, Gable::new(X, [3, 7], [2, 4], 3, 3));
    c.place(&K.log_x, 5, 5, 5);
    c.door(SPRUCE_DOOR, [3, 1, 5], East, Hinge::Left);
    c.place(&stairs(COBBLE_STAIRS, East), 2, 0, 5);

    for z in [3, 7] {
        c.place(&S.grass, 1, 0, z);
        c.place(&S.poppy, 1, 1, z);
        trapdoor_ring(c, 0, [1, z], [1, z], "top");
        shuttered_window(c, [8, 2, z], East, "top");
        c.place(&S.wall_torch, 7, 3, z);
    }
    shuttered_window(c, [8, 2, 5], East, "top");
    for z in [2, 8] {
        shuttered_window(c, [5, 2, z], West, "top");
    }
    c.solid(&stairs(SPRUCE_STAIRS, North), [6, 1, 2], [7, 1, 2]);
    c.solid(&stairs(SPRUCE_STAIRS, South), [6, 1, 8], [7, 1, 8]);
    chest(c, [5, 1, 4], East, "village_fletcher");
    c.fill(&S.spruce_fence, 7, 1, [4, 6]);
    c.fill(&K.purple_carpet, 7, 2, [4, 6]);
    c.place(&block("minecraft:fletching_table"), 7, 1, 5);
    c.fill(&S.wall_torch, 2, 2, [4, 6]);
    c.place(&wall_torch(East), 4, 3, 5);
    c.entrance([0, 0, 5], EMPTY, NOTHING);
}

/// Wheat on the layer `y`: `[x, z, age]` of each plant.
fn wheat_field(c: &mut Canvas, y: i32, plants: &[[i32; 3]]) {
    for [x, z, age] in plants {
        c.place(&block(&format!("minecraft:wheat[age={age}]")), *x, y, *z);
    }
}

/// Pumpkins on the layer `y`, each on a block of dirt.
fn pumpkins(c: &mut Canvas, y: i32, cells: &[[i32; 2]]) {
    c.scatter(&S.dirt, y - 1, cells);
    c.scatter(&K.pumpkin, y, cells);
}

fn taiga_large_farm_1(c: &mut Canvas) {
    let grass = [
        2, 5, 1, 1, 6, 2, 8, 8, 2, 3, 5, 3, 7, 8, 3, 5, 8, 4, 1, 2, 5, 8, 8, 5, 1, 1, 6, 3, 6, 6,
        1, 7, 7, 1, 1, 8, 4, 8, 8, 8, 8, 9,
    ];
    let path = [1, 1, 3, 1, 4, 4, 3, 7, 5, 7, 9, 6];
    let dirt = [
        7, 0, 1, 1, 9, 2, 0, 3, 2, 3, 6, 3, 0, 4, 2, 6, 0, 7, 8, 7, 4, 9,
    ];
    let farmland = [
        2, 5, 1, 1, 2, 2, 4, 6, 2, 3, 3, 3, 5, 5, 3, 7, 7, 3, 5, 8, 4, 1, 2, 5, 8, 8, 5, 1, 1, 6,
        3, 4, 6, 6, 6, 6, 2, 5, 7, 7, 7, 7, 6, 8, 8,
    ];
    let plants = [
        2, 1, 4, 3, 1, 7, 4, 1, 5, 5, 1, 7, 1, 2, 7, 2, 2, 5, 4, 2, 7, 5, 2, 5, 6, 2, 4, 3, 3, 1,
        5, 3, 6, 7, 3, 5, 5, 4, 7, 6, 4, 7, 7, 4, 7, 8, 4, 7, 1, 5, 7, 2, 5, 7, 8, 5, 7, 1, 6, 1,
        3, 6, 7, 4, 6, 3, 6, 6, 4, 2, 7, 3, 3, 7, 5, 4, 7, 7, 5, 7, 3, 7, 7, 7, 6, 8, 7, 7, 8, 5,
        8, 8, 7,
    ];
    c.void([0, 5, 0], [9, 6, 9]);
    c.runs(&S.grass, 0, grass.as_chunks().0);
    c.runs(&S.path, 0, path.as_chunks().0);
    c.scatter(&S.dirt, 0, dirt.as_chunks().0);
    c.runs(&S.farmland, 1, farmland.as_chunks().0);
    c.scatter(&S.water, 1, &[[3, 2], [6, 3], [2, 6], [6, 7]]);
    let north = [[1, 5, 0], [6, 6, 1], [7, 7, 2], [8, 8, 3]];
    c.runs(&trapdoor(North, "bottom"), 1, &north);
    c.runs(
        &trapdoor(South, "bottom"),
        1,
        &[[1, 1, 7], [2, 5, 8], [6, 8, 9]],
    );
    c.fill(&trapdoor(West, "bottom"), 0, 1, [1, 2, 5, 6]);
    c.fill(&trapdoor(East, "bottom"), 9, 1, [4, 5, 7, 8]);
    wheat_field(c, 2, plants.as_chunks().0);
    c.scatter(&K.pumpkin, 1, &[[7, 0], [9, 2], [0, 4], [0, 7], [4, 9]]);
    pumpkins(c, 2, &[[1, 1], [8, 7]]);
    c.place(&S.composter, 2, 1, 3);
    c.place(&stairs(COBBLE_STAIRS, North), 4, 1, 3);
    c.place(&stairs(COBBLE_STAIRS, South), 5, 1, 6);
    c.torch_post(&S.cobble_wall, [8, 1, 2], 3);
    c.torch_post(&S.cobble_wall, [1, 1, 8], 3);
    c.entrance([0, 1, 3], EMPTY, NOTHING);
    c.spot([8, 0, 1], CATS, GRASS);
}

fn large_farm_2(c: &mut Canvas, v: Village) {
    let grass = [
        1, 2, 0, 1, 6, 1, 1, 2, 2, 4, 5, 2, 1, 5, 3, 2, 5, 4, 3, 3, 5, 5, 5, 5, 7, 7, 5, 0, 1, 6,
        4, 5, 6, 0, 0, 7, 4, 6, 8,
    ];
    let path = [0, 1, 4, 1, 2, 5, 2, 3, 6, 3, 7, 7];
    let dirt = [5, 0, 0, 1, 3, 2, 7, 2, 0, 5, 4, 5, 2, 7, 3, 8];
    let farmland = [
        2, 3, 1, 1, 2, 2, 4, 4, 2, 1, 5, 3, 3, 5, 4, 3, 3, 5, 5, 5, 5, 4, 5, 6,
    ];
    let plants = [
        2, 1, 3, 3, 1, 7, 1, 2, 7, 2, 2, 7, 4, 2, 0, 1, 3, 7, 2, 3, 7, 3, 3, 1, 4, 3, 7, 5, 3, 7,
        3, 4, 0, 4, 4, 7, 5, 4, 7, 3, 5, 7, 5, 5, 0, 4, 6, 7, 5, 6, 0,
    ];
    c.void([0, 4, 0], [7, 6, 8]);
    c.runs(&S.grass, 0, grass.as_chunks().0);
    c.runs(&S.path, 0, path.as_chunks().0);
    c.scatter(&S.dirt, 0, dirt.as_chunks().0);
    c.runs(&S.farmland, 1, farmland.as_chunks().0);
    c.scatter(&S.water, 1, &[[3, 2], [4, 5]]);
    c.solid(&trapdoor(North, "bottom"), [2, 1, 0], [3, 1, 0]);
    c.scatter(&trapdoor(West, "bottom"), 1, &[[1, 1], [0, 2], [0, 3]]);
    c.scatter(&trapdoor(East, "bottom"), 1, &[[4, 1], [5, 2]]);
    c.solid(&trapdoor(East, "bottom"), [6, 1, 3], [6, 1, 6]);
    wheat_field(c, 2, plants.as_chunks().0);
    c.scatter(&K.pumpkin, 1, &[[5, 0], [0, 1], [7, 2], [0, 5], [2, 7]]);
    c.scatter(&K.large_fern, 1, &[[1, 0], [6, 1], [0, 7], [4, 8]]);
    c.place(&K.fern, 7, 1, 5);
    c.place(&S.composter, 5, 1, 8);
    c.place(&stairs(COBBLE_STAIRS, North), 2, 1, 4);
    c.torch_post(&S.cobble_wall, [1, 1, 6], 2);
    c.torch_post(&wall_post(&[], &[]), [6, 1, 8], 2);
    c.entrance([3, 1, 8], EMPTY, NOTHING);
    decorations(c, v, GRASS, &[[7, 6]]);
}

fn taiga_small_farm_1(c: &mut Canvas) {
    let beds = [2, 2, 1, 4, 4, 1, 1, 5, 2, 1, 5, 4, 3, 4, 5, 1, 3, 6];
    let mossy = [
        0, 2, 0, 4, 5, 0, 3, 3, 1, 0, 0, 2, 6, 6, 2, 0, 0, 3, 4, 4, 3, 6, 6, 3, 1, 1, 5, 5, 6, 5,
        0, 0, 6, 2, 2, 7, 4, 6, 7,
    ];
    let stem = block("minecraft:pumpkin_stem[age=7]");
    c.void([0, 4, 0], [6, 6, 7]);
    c.solid(&S.dirt, [0, 0, 0], [6, 0, 7]);
    c.solid(&S.cobble, [0, 1, 0], [6, 1, 7]);
    c.runs(&S.mossy, 1, mossy.as_chunks().0);
    for (y, layer) in [&S.grass, &S.farmland, &stem].into_iter().enumerate() {
        c.runs(layer, y as i32, beds.as_chunks().0);
    }
    c.scatter(&S.water, 1, &[[1, 1], [5, 1], [3, 3], [2, 5], [5, 6]]);
    c.place(&block("minecraft:water[level=8]"), 2, 0, 5);
    pumpkins(c, 2, &[[1, 3], [2, 3], [5, 3], [4, 6]]);
    c.place(&S.composter, 4, 2, 3);
    for x in [2, 4] {
        c.torch_post(&S.cobble_wall, [x, 2, 0], 1);
    }
    c.entrance([3, 1, 0], EMPTY, STEP_SOUTH);
    c.spot([6, 1, 0], CATS, COBBLE);
}

fn library_1(c: &mut Canvas, v: Village) {
    c.void([0, 0, 1], [10, 0, 7]);
    c.solid(&GROUND, [1, 0, 0], [9, 0, 6]);
    c.place(&GROUND, 8, 0, 7);
    c.solid(&S.path, [5, 0, 0], [5, 0, 1]);
    c.solid(&S.cobble, [2, 0, 3], [8, 0, 5]);
    c.place(&S.cobble, 5, 0, 2);

    c.walls(&S.cobble, &S.cobble, [1, 1, 2], [9, 2, 6]);
    c.solid(&S.cobble, [1, 3, 2], [9, 3, 6]);
    c.solid(&S.air, [2, 3, 4], [4, 3, 4]);
    for step in 0..3 {
        c.place(&stairs(COBBLE_STAIRS, East), 3 + step, 1 + step, 4);
        c.solid(&S.cobble, [4 + step, 1 + step, 4], [5, 1 + step, 4]);
    }
    c.door(SPRUCE_DOOR, [5, 1, 2], South, Hinge::Right);
    c.fill(&S.pane, [3, 7], 2, 2);
    c.fill(&S.pane, [1, 9], 2, 4);
    c.fill(&S.pane, [3, 5, 7], 2, 6);
    c.solid(&block("minecraft:red_carpet"), [7, 1, 3], [7, 1, 5]);
    c.fill(&stairs(SPRUCE_STAIRS, East), 8, 1, [3, 4]);
    c.lectern([8, 1, 5], West);
    c.place(&S.wall_torch, 6, 2, 3);
    c.place(&wall_torch(North), 2, 2, 5);
    c.place(&S.wall_torch, 6, 2, 5);

    for (x, fern, flower) in [(1, 1, 2), (7, 9, 8)] {
        c.solid(&S.grass, [x, 1, 1], [x + 2, 1, 1]);
        c.solid(&trapdoor(North, "bottom"), [x, 1, 0], [x + 2, 1, 0]);
        c.place(&trapdoor(West, "bottom"), x - 1, 1, 1);
        c.place(&trapdoor(East, "bottom"), x + 3, 1, 1);
        c.place(&K.large_fern, fern, 2, 1);
        c.place(&trapdoor(North, "bottom"), flower, 2, 1);
    }
    c.place(&S.poppy, 7, 2, 1);
    for x in [4, 6] {
        c.torch_post(&S.cobble_wall, [x, 1, 0], 1);
    }
    c.place(&K.fern, 8, 1, 7);

    c.walls(&S.cobble, &S.cobble, [1, 4, 2], [9, 4, 6]);
    gabled_roof(c, Gable::new(Z, [0, 10], [1, 7], 4, 6), &S.cobble, [2, 6]);
    c.solid(&S.grass, [4, 4, 1], [6, 4, 1]);
    c.solid(&trapdoor(North, "bottom"), [4, 4, 0], [6, 4, 0]);
    c.place(&trapdoor(West, "bottom"), 3, 4, 1);
    c.place(&trapdoor(East, "bottom"), 7, 4, 1);
    for x in [4, 6] {
        shuttered_window(c, [x, 5, 2], North, "bottom");
    }
    c.place(&S.poppy, 4, 5, 1);
    c.fill(&S.pane, [3, 5, 7], 5, 6);
    c.solid(&K.purple_carpet, [6, 4, 3], [7, 4, 5]);
    c.solid(&S.bookshelf, [8, 4, 3], [8, 5, 5]);
    c.fill(&S.wall_torch, [3, 7], 6, 4);
    c.fill(&S.wall_torch, 5, 7, [1, 3, 5, 7]);
    c.entrance([5, 1, 0], EMPTY, NOTHING);
    decorations(c, v, GRASS, &[[0, 0]]);
    spots(c, CATS, GRASS, &[[1, 0, 7], [3, 0, 7]]);
}

fn taiga_masons_house_1(c: &mut Canvas) {
    c.void([2, 0, 0], [6, 0, 0]);
    c.void([7, 0, 1], [7, 0, 7]);
    c.solid(&S.grass, [0, 0, 0], [1, 0, 8]);
    c.solid(&S.grass, [2, 0, 1], [2, 0, 8]);
    c.solid(&S.grass, [3, 0, 8], [7, 0, 8]);
    c.place(&S.grass, 7, 0, 0);
    c.boxes(&S.dirt, &[([3, 0, 1], [6, 0, 7]), ([2, 0, 3], [2, 0, 5])]);
    c.place(&S.dirt, 1, 0, 4);

    for z in [1, 7] {
        c.solid(&S.cobble, [3, 1, z], [6, 2, z]);
        c.solid(&S.spruce_fence, [4, 2, z], [5, 2, z]);
    }
    c.fill(&S.cobble, 3, 1..=3, [2, 6]);
    c.solid(&S.cobble, [2, 1, 3], [2, 3, 5]);
    c.solid(&S.cobble, [6, 1, 2], [6, 3, 6]);
    for x in [2, 6] {
        c.solid(&S.cobble, [x, 4, 3], [x, 4, 5]);
        c.place(&S.cobble, x, 5, 4);
    }
    c.solid(&S.spruce_planks, [3, 1, 3], [5, 1, 5]);
    c.fill(&S.spruce_planks, 4..=5, 1, [2, 6]);
    log_roof(c, Gable::new(X, [0, 8], [1, 7], 2, 5));
    c.posts(&S.spruce_fence, &[1, 7], &[0, 8], [1, 1]);
    c.door(SPRUCE_DOOR, [2, 2, 4], East, Hinge::Right);
    shuttered_window(c, [6, 3, 4], East, "top");

    c.place(&block("minecraft:potted_spruce_sapling"), 3, 2, 3);
    c.place(&block("minecraft:stonecutter[facing=north]"), 5, 2, 4);
    c.place(&stairs(COBBLE_STAIRS, South), 3, 2, 5);
    c.fill(&S.wall_torch, 1, 3, [3, 5]);
    c.place(&S.wall_torch, 5, 4, 4);
    for z in [2, 6] {
        planter(c, [1, 1, z]);
    }
    west_entrance(c, [1, 1, 4], STEP_EAST);
}

fn shepherds_house_1(c: &mut Canvas, v: Village) {
    c.void([0, 0, 0], [9, 0, 10]);
    c.void([0, 1, 0], [0, 1, 1]);
    c.void([0, 1, 8], [0, 1, 8]);
    c.void([1, 1, 6], [1, 1, 6]);
    c.solid(&S.dirt, [2, 0, 2], [9, 0, 8]);
    c.fill(&S.dirt, 1..=5, 0, [1, 9]);
    c.fill(&S.dirt, 1, 0, [3, 5]);
    c.place(&S.grass, 1, 0, 7);
    c.place(&v.underfoot(), 1, 0, 3);
    for z in [3, 7] {
        c.place(&S.path, 0, 0, z);
        c.place(&stairs(COBBLE_STAIRS, East), 1, 1, z);
    }

    c.walls(&S.cobble, &S.cobble, [5, 1, 2], [9, 1, 8]);
    c.solid(&S.grass, [6, 1, 3], [8, 1, 7]);
    c.walls(&S.spruce_fence, &S.spruce_fence, [5, 2, 2], [9, 2, 8]);
    c.fill(&S.torch, 9, 3, [2, 8]);
    c.spot([8, 1, 6], "minecraft:village/common/sheep", GRASS);

    c.walls(&S.cobble, &S.cobble, [2, 1, 1], [5, 2, 9]);
    c.solid(&S.spruce_planks, [3, 1, 2], [4, 1, 8]);
    c.walls(&S.spruce_planks, &S.spruce_planks, [2, 3, 1], [5, 3, 9]);
    c.fill(&S.spruce_planks, [2, 5], 4, [2..=4, 6..=8]);
    c.fill(&S.spruce_planks, [2, 5], 5, [3, 7]);
    c.posts(&K.log, &[1], &[1, 5, 9], [1, 3]);
    c.fill(&K.log_z, 1, 4, [2..=4, 6..=8]);
    c.fill(&K.log_x, 0..=6, 3, [0, 10]);
    for z in [1, 5] {
        log_roof(c, Gable::new(X, [z, z + 4], [0, 6], 4, 3));
    }
    c.door(SPRUCE_DOOR, [2, 2, 3], East, Hinge::Left);
    c.door(SPRUCE_DOOR, [2, 2, 7], East, Hinge::Right);
    c.door(SPRUCE_DOOR, [5, 2, 5], West, Hinge::Right);
    for z in [3, 7] {
        shuttered_window(c, [5, 3, z], East, "top");
        c.place(&wall_torch(West), 1, 5, z);
    }

    c.solid(&block("minecraft:loom[facing=south]"), [3, 2, 2], [4, 2, 2]);
    let white = block("minecraft:white_carpet");
    for z in 4..=6 {
        c.place(&K.purple_carpet, 3 + z % 2, 2, z);
        c.place(&white, 4 - z % 2, 2, z);
    }
    c.solid(&S.spruce_fence, [3, 2, 8], [4, 2, 8]);
    c.solid(&K.plate, [3, 3, 8], [4, 3, 8]);
    c.place(&wall_torch(North), 3, 4, 4);
    c.place(&wall_torch(South), 3, 4, 6);
    c.entrance([0, 1, 5], EMPTY, NOTHING);
    decorations(c, v, GRASS, &[[8, 0]]);
    if v.zombie {
        c.each(&S.air, &[[0, 1, 0], [0, 1, 1], [1, 1, 6]]);
        c.open_door("minecraft:spruce_door", [2, 2, 3], South, Hinge::Right);
    }
}

fn taiga_tannery_1(c: &mut Canvas) {
    c.solid(&S.cobble, [2, 0, 1], [7, 0, 7]);
    c.solid(&S.grass, [1, 0, 1], [1, 0, 4]);
    c.place(&trapdoor(North, "top"), 1, 0, 0);
    c.solid(&trapdoor(West, "top"), [0, 0, 1], [0, 0, 4]);
    c.fill(&K.large_fern, 1, 1, [1, 3]);
    c.place(&S.poppy, 1, 1, 2);
    c.place(&K.fern, 1, 1, 4);
    c.place(&stairs(COBBLE_STAIRS, East), 1, 0, 6);
    c.place(&S.cobble, 1, 0, 7);
    c.fill(&S.torch, 1, 1, [5, 7]);

    c.walls(&S.cobble, &S.cobble, [2, 1, 1], [7, 2, 7]);
    c.solid(&S.cobble, [4, 4, 4], [5, 4, 4]);
    gabled_roof(c, Gable::new(X, [0, 8], [1, 8], 2, 4), &S.cobble, [2, 7]);
    c.solid(&K.log_x, [1, 5, 4], [8, 5, 4]);
    c.door(SPRUCE_DOOR, [2, 1, 6], East, Hinge::Left);
    shuttered_window(c, [2, 2, 3], West, "top");
    for z in [3, 5] {
        shuttered_window(c, [7, 2, z], East, "top");
    }

    trapdoor_ring(c, 1, [4, 3], [5, 3], "bottom");
    chest(c, [6, 1, 2], South, "village_tannery");
    c.solid(
        &block("minecraft:water_cauldron[level=3]"),
        [5, 1, 6],
        [6, 1, 6],
    );
    c.place(&wall_torch(West), 3, 4, 4);
    c.place(&wall_torch(East), 6, 4, 4);
    west_entrance(c, [1, 0, 5], COBBLE);
}

fn temple_1(c: &mut Canvas) {
    let ground = [
        8, 8, 0, 10, 12, 0, 8, 12, 1, 4, 12, 2, 4, 12, 3, 1, 12, 4, 1, 12, 5, 1, 10, 6, 1, 9, 7, 1,
        9, 8, 0, 6, 9, 0, 7, 10,
    ];
    c.void([0, 0, 0], [12, 0, 10]);
    c.void([0, 1, 4], [0, 1, 4]);
    c.each(&S.air, &[[1, 0, 0], [0, 0, 4], [7, 0, 9]]);
    c.runs(&GROUND, 0, ground.as_chunks().0);
    c.boxes(&S.cobble, &[([7, 0, 3], [9, 0, 4]), ([2, 0, 5], [9, 0, 5])]);
    c.boxes(&S.cobble, &[([2, 0, 6], [6, 0, 7]), ([5, 0, 8], [6, 0, 8])]);
    c.place(&S.spruce_planks, 11, 0, 1);
    c.place(&S.path, 6, 0, 9);
    c.solid(&S.path, [5, 0, 10], [7, 0, 10]);

    c.boxes(&S.cobble, &[([1, 1, 4], [6, 3, 4]), ([1, 1, 5], [1, 3, 7])]);
    c.boxes(&S.cobble, &[([1, 1, 8], [7, 3, 8]), ([7, 1, 7], [7, 4, 7])]);
    let vault = Gable::new(X, [5, 7], [1, 6], 4, 2);
    c.stepped_gable(vault, &S.cobble, None, Some(&S.cobble));
    c.place(&S.cobble, 1, 4, 6);
    c.door(SPRUCE_DOOR, [6, 1, 8], North, Hinge::Right);
    for x in [2, 4] {
        shuttered_window(c, [x, 2, 4], North, "top");
    }
    shuttered_window(c, [1, 2, 6], West, "top");
    shuttered_window(c, [3, 2, 8], South, "top");
    c.fill(&stairs(COBBLE_STAIRS, West), 2, 1, [5, 7]);
    c.brewing_stand([2, 1, 6]);
    c.place(&wall_torch(East), 2, 4, 6);
    c.place(&wall_torch(West), 6, 4, 6);

    c.boxes(
        &S.cobble,
        &[([7, 1, 2], [9, 10, 2]), ([7, 1, 6], [9, 10, 6])],
    );
    c.boxes(
        &S.cobble,
        &[([6, 1, 3], [6, 10, 5]), ([10, 1, 3], [10, 10, 5])],
    );
    c.solid(&S.air, [6, 1, 5], [6, 2, 5]);
    c.solid(&S.cobble, [7, 5, 3], [9, 5, 5]);
    let ladder = block("minecraft:ladder[facing=south,waterlogged=false]");
    c.solid(&ladder, [7, 2, 3], [7, 5, 3]);
    for y in [7, 9] {
        for (at, out) in [
            ([8, y, 2], North),
            ([8, y, 6], South),
            ([6, y, 4], West),
            ([10, y, 4], East),
        ] {
            c.place(&S.spruce_fence, at[0], at[1], at[2]);
            shutters(c, at, out, "bottom");
        }
    }
    c.fill(&K.purple_carpet, 8..=9, 1, [3, 5]);
    c.place(&K.purple_carpet, 8, 1, 4);
    c.place(&S.cobble, 9, 1, 4);
    c.place(&block("minecraft:potted_poppy"), 9, 2, 4);
    c.place(&wall_torch(East), 7, 3, 5);
    c.fill(&S.wall_torch, [7, 9], 10, 4);

    let wood = |axis| log("minecraft:spruce_wood", axis);
    for z in [1, 7] {
        c.solid(&K.log_x, [7, 11, z], [9, 11, z]);
        c.fill(&wood(X), [6, 10], 11, z);
    }
    c.fill(&K.log_z, [5, 11], 11, 3..=5);
    c.fill(&wood(Z), 5, 11, [2, 6]);
    c.fill(&wood(Y), 11, 11, [2, 6]);
    c.fill(&K.log_x, 6..=10, 11, [2, 6]);
    c.fill(&K.log_z, [6, 10], 11, 3..=5);
    c.place(&K.log_z, 6, 11, 2);
    c.each(&K.log, &[[9, 11, 2], [10, 11, 4]]);
    c.fill(&K.log_x, 7..=9, 12, [2, 6]);
    c.fill(&K.log_z, [6, 10], 12, 3..=5);
    c.each(
        &wood(X),
        &[[6, 12, 2], [10, 12, 2], [10, 12, 6], [9, 12, 4]],
    );
    c.each(&wood(Z), &[[6, 12, 6], [8, 12, 3]]);
    c.each(&K.log_x, &[[7, 12, 3], [9, 12, 3], [7, 12, 5], [8, 12, 5]]);
    c.place(&K.log_z, 9, 12, 5);
    c.solid(&wood(X), [8, 13, 3], [8, 13, 5]);
    c.fill(&wood(Y), [7, 9], 13, [3, 5]);
    c.fill(&wood(Z), [7, 9], 13, 4);

    c.solid(&S.grass, [2, 1, 9], [4, 1, 9]);
    c.place(&trapdoor(West, "bottom"), 1, 1, 9);
    c.place(&trapdoor(East, "bottom"), 5, 1, 9);
    c.solid(&trapdoor(South, "bottom"), [2, 1, 10], [4, 1, 10]);
    c.place(&S.poppy, 3, 2, 9);
    c.solid(&S.grass, [8, 1, 7], [8, 1, 8]);
    c.solid(&trapdoor(East, "bottom"), [9, 1, 7], [9, 1, 8]);
    c.place(&trapdoor(South, "bottom"), 8, 1, 9);
    c.place(&K.large_fern, 8, 2, 7);
    c.place(&S.poppy, 8, 2, 8);
    planter(c, [11, 1, 1]);
    let ferns = [[12, 0], [4, 2], [10, 2], [12, 5], [10, 6]];
    c.scatter(&K.large_fern, 1, &ferns);

    c.solid(&S.cobble, [8, 1, 0], [8, 2, 1]);
    c.solid(&S.cobble, [11, 1, 4], [12, 1, 4]);
    c.place(&stairs(COBBLE_STAIRS, East), 11, 2, 4);
    c.place(&S.cobble, 12, 2, 4);
    for at in [[8, 3, 0], [12, 3, 4], [0, 1, 10]] {
        c.torch_post(&S.cobble_wall, at, 1);
    }
    c.entrance([6, 1, 10], EMPTY, NOTHING);
}

fn tool_smith_1(c: &mut Canvas, v: Village) {
    c.void([0, 0, 0], [10, 0, 7]);
    c.fill(&VOID, [1, 4], 1, 0);
    c.solid(&S.dirt, [1, 0, 1], [9, 0, 4]);
    c.solid(&S.cobble, [2, 0, 2], [8, 0, 3]);
    c.place(&S.cobble, 5, 0, 4);
    c.fill(&S.dirt, [4, 6], 0, 5..=6);
    c.solid(&S.spruce_planks, [5, 0, 5], [5, 0, 6]);
    c.place(&S.path, 5, 0, 7);

    c.walls(&S.cobble, &S.cobble, [1, 1, 1], [9, 3, 4]);
    gabled_roof(c, Gable::new(X, [0, 5], [0, 10], 3, 3), &S.cobble, [1, 9]);
    c.place(&S.air, 5, 3, 5);
    c.fill(&S.spruce_planks, [4, 6], 1..=3, 5..=6);
    c.place(&S.spruce_planks, 5, 3, 6);
    c.fill(&S.spruce_planks, 5, 4, 5..=6);
    c.fill(&K.log_z, [3, 7], 3, 6..=7);
    log_roof(c, Gable::new(Z, [4, 6], [4, 7], 4, 2));
    for z in [4, 6] {
        c.door(SPRUCE_DOOR, [5, 1, z], North, Hinge::Right);
    }
    for x in [3, 5, 7] {
        shuttered_window(c, [x, 2, 1], North, "top");
    }
    for x in [2, 8] {
        shuttered_window(c, [x, 2, 4], South, "top");
    }

    c.place(&block("minecraft:smithing_table"), 2, 1, 2);
    c.place(&S.cobble, 2, 1, 3);
    chest(c, [6, 1, 3], North, "village_toolsmith");
    c.fill(&stairs(COBBLE_STAIRS, East), 8, 1, 2..=3);
    c.place(&S.wall_torch, 5, 3, 3);
    c.place(&S.wall_torch, 5, 3, 7);
    c.fill(&wall_torch(East), 2, 4, 2..=3);
    c.fill(&wall_torch(West), 8, 4, 2..=3);
    c.entrance([5, 1, 7], EMPTY, NOTHING);
    decorations(c, v, GRASS, &[[1, 7]]);
    c.spot([9, 0, 7], CATS, GRASS);
}

fn taiga_weaponsmith_1(c: &mut Canvas) {
    c.solid(&GROUND, [0, 0, 0], [6, 0, 6]);
    c.solid(&S.path, [1, 0, 0], [5, 0, 0]);
    c.fill(&S.path, [1, 3, 5], 0, 1);
    c.solid(&S.spruce_planks, [2, 0, 3], [4, 0, 4]);
    c.place(&S.spruce_planks, 3, 0, 2);
    c.fill(&K.grindstone, [2, 4], 1, 1);
    c.scatter(&K.fern, 1, &[[6, 0], [0, 2]]);

    c.walls(&S.cobble, &S.cobble, [1, 1, 2], [5, 3, 5]);
    c.walls(&S.spruce_planks, &S.spruce_planks, [1, 4, 2], [5, 5, 5]);
    gabled_roof(
        c,
        Gable::new(Z, [0, 6], [1, 6], 5, 4),
        &S.spruce_planks,
        [2, 5],
    );
    c.door(SPRUCE_DOOR, [3, 1, 2], South, Hinge::Left);
    for x in [2, 4] {
        shuttered_window(c, [x, 2, 5], South, "top");
        shuttered_window(c, [x, 5, 5], South, "top");
        c.place(&S.pane, x, 5, 2);
    }

    c.solid(&S.grass, [1, 4, 1], [5, 4, 1]);
    c.solid(&trapdoor(North, "top"), [1, 4, 0], [5, 4, 0]);
    c.place(&trapdoor(West, "bottom"), 0, 4, 1);
    c.place(&trapdoor(East, "top"), 6, 4, 1);
    c.fill(&S.poppy, [1, 4], 5, 1);
    c.fill(&K.fern, [3, 5], 5, 1);

    chest(c, [2, 1, 3], East, "village_weaponsmith");
    c.place(&S.spruce_fence, 2, 1, 4);
    c.place(&shut_trapdoor(East, "bottom"), 2, 2, 4);
    c.fill(&stairs(SPRUCE_STAIRS, East), 4, 1, 3..=4);
    c.place(&S.wall_torch, 3, 3, 1);
    c.place(&S.wall_torch, 3, 5, 4);
    c.place(&S.wall_torch, 3, 6, 1);
    c.entrance([3, 1, 0], EMPTY, NOTHING);
}

fn weaponsmith_2(c: &mut Canvas, v: Village) {
    let path = [
        1, 1, 0, 3, 4, 0, 2, 3, 1, 1, 3, 2, 1, 1, 3, 3, 3, 3, 2, 2, 4,
    ];
    let under_roof = wall_post(&[South], &[]);
    c.void([5, 0, 0], [5, 0, 6]);
    c.void([5, 1, 4], [5, 1, 4]);
    c.solid(&S.grass, [0, 0, 0], [4, 0, 5]);
    c.place(&S.grass, 5, 0, 1);
    c.solid(&S.dirt, [0, 0, 6], [4, 0, 6]);
    c.runs(&S.path, 0, path.as_chunks().0);
    c.place(&S.cobble, 2, 0, 3);
    for step in 0..4 {
        c.solid(&K.log_x, [0, 1 + step, 6 - step], [4, 1 + step, 6 - step]);
    }
    for x in [0, 4] {
        c.solid(&S.cobble_wall, [x, 1, 1], [x, 1, 3]);
        c.place(&S.torch, x, 2, 1);
        c.place(&S.cobble_wall, x, 2, 3);
        c.place(&under_roof, x, 3, 3);
    }
    c.fill(&under_roof, [0, 2, 4], 1, 5);
    c.place(&K.grindstone, 2, 1, 3);
    c.scatter(&K.fern, 1, &[[0, 0], [1, 4], [4, 4], [1, 5]]);
    c.scatter(&K.large_fern, 1, &[[5, 1], [3, 4]]);
    c.entrance([2, 1, 0], EMPTY, NOTHING);
    decorations(c, v, GRASS, &[[5, 2]]);
}

fn corner_02_of(c: &mut Canvas, v: Village) {
    let ends = [[1, 0], [15, 14]];
    street(c, v, &ends);
    path(c, Z, 1, [0, 12]);
    c.solid(&S.path, [1, 0, 13], [15, 0, 14]);
    c.solid(&S.path, [3, 0, 15], [15, 0, 15]);
    c.scatter(&S.dirt, 0, &ends);
    c.place(&S.dirt, 9, 0, 12);
    houses(c, v, North, 12, [9, 9]);
    decorations(c, v, GRASS, &[[1, 3], [0, 15]]);
}

fn crossroad_04_of(c: &mut Canvas, v: Village) {
    let ends = [[2, 0], [0, 2], [2, 4]];
    street(c, v, &ends);
    path(c, Z, 2, [0, 4]);
    c.solid(&S.path, [0, 0, 1], [0, 0, 3]);
    c.scatter(&S.dirt, 0, &ends);
    decorations(c, v, GRASS, &[[3, 2]]);
}

/// A street along the west edge, `length` long, with places for houses on
/// its east side over `sockets` and decorations in its middle at `decor`.
fn straight_with_houses(c: &mut Canvas, v: Village, length: i32, sockets: [i32; 2], decor: &[i32]) {
    street(c, v, &[[1, 0], [1, length - 1]]);
    path(c, Z, 1, [0, length - 1]);
    houses(c, v, East, 2, sockets);
    for z in decor {
        decorations(c, v, GRASS, &[[1, *z]]);
    }
}

fn straight_04_of(c: &mut Canvas, v: Village) {
    straight_with_houses(c, v, 9, [4, 4], &[]);
    decorations(c, v, GRASS, &[[0, 4]]);
}

fn taiga_decoration_1(c: &mut Canvas) {
    c.void([0, 0, 0], [2, 0, 5]);
    c.solid(&S.spruce_planks, [1, 0, 1], [1, 0, 4]);
    trapdoor_ring(c, 1, [1, 1], [1, 4], "bottom");
    marker(c, [1, 1, 3], NOTHING);
}

fn taiga_decoration_2(c: &mut Canvas) {
    c.place(&stairs(COBBLE_STAIRS, South), 1, 0, 0);
    marker(c, [1, 0, 1], COBBLE);
    c.place(&stairs(COBBLE_STAIRS, East), 0, 0, 2);
    c.place(&S.cobble, 1, 0, 2);
    c.place(&stairs(COBBLE_STAIRS, South), 1, 1, 1);
    c.place(&stairs(COBBLE_STAIRS, North), 1, 1, 2);
}

/// A cobblestone step with `top` on the block behind it.
fn stone_seat(c: &mut Canvas, top: &Cell) {
    marker(c, [0, 0, 0], COBBLE);
    c.place(&stairs(COBBLE_STAIRS, North), 0, 0, 1);
    c.place(top, 0, 1, 0);
}

fn taiga_decoration_5(c: &mut Canvas) {
    marker(
        c,
        [0, 0, 0],
        "minecraft:campfire[lit=true,signal_fire=false]",
    );
}

fn taiga_decoration_6(c: &mut Canvas) {
    trapdoor_ring(c, 0, [1, 1], [1, 1], "bottom");
    marker(c, [1, 0, 1], "minecraft:hay_block");
    campfire(c, [1, 1, 1], East, true);
}

pub fn taiga_lamp_post_1(c: &mut Canvas) {
    marker(c, [0, 0, 0], "minecraft:cobblestone_wall");
    c.place(&S.torch, 0, 1, 0);
}

fn meeting_point_1(c: &mut Canvas, v: Village) {
    let path = [
        5, 7, 0, 1, 2, 1, 4, 7, 1, 10, 11, 1, 0, 1, 2, 3, 5, 2, 7, 8, 2, 10, 10, 2, 0, 0, 3, 2, 2,
        3, 4, 4, 3, 6, 7, 3, 9, 9, 3, 11, 11, 3, 0, 8, 4, 10, 11, 4, 2, 2, 5, 4, 5, 5, 7, 7, 5, 9,
        11, 5, 2, 8, 6, 10, 11, 6,
    ];
    let grass = [
        3, 1, 8, 1, 9, 1, 2, 2, 6, 2, 9, 2, 11, 2, 1, 3, 8, 3, 10, 3, 9, 4, 1, 5, 3, 5, 6, 5, 8, 5,
        1, 6, 9, 6,
    ];
    c.runs(&S.path, 0, path.as_chunks().0);
    c.scatter(&S.grass, 0, grass.as_chunks().0);
    c.place(&S.spruce_planks, 9, 1, 3);
    trapdoor_ring(c, 1, [9, 3], [9, 3], "bottom");
    c.bell([9, 2, 3], "floor", North);
    street_ends(c, v, &[[6, 0], [0, 3]]);
    houses(c, v, South, 6, [2, 7]);
    houses(c, v, East, 11, [3, 6]);
    spots(c, CATS, GRASS, &[[3, 0, 3], [5, 0, 3]]);
    if v.zombie {
        let bare = [[6, 0], [9, 3], [11, 3], [2, 6], [5, 6], [7, 6], [11, 6]];
        c.scatter(&S.dirt, 0, &bare);
        open(c, 1..=5, &[[3, 1, 3], [4, 1, 3], [5, 1, 3]]);
    } else {
        villagers(c, v, PATH, &[[2, 0, 4], [7, 0, 2], [9, 0, 5]]);
        c.spot([6, 0, 4], IRON_GOLEM, PATH);
    }
}

fn meeting_point_2(c: &mut Canvas, v: Village) {
    let soil = [
        0, 0, 0, 3, 5, 0, 1, 7, 1, 1, 7, 2, 0, 8, 3, 0, 8, 4, 0, 8, 5, 1, 7, 6, 1, 8, 7, 3, 4, 8,
    ];
    let overgrown = [
        3, 3, 0, 5, 5, 0, 1, 4, 1, 6, 6, 1, 7, 7, 2, 0, 1, 3, 7, 8, 3, 0, 1, 4, 7, 8, 4, 0, 0, 5,
        7, 7, 6, 1, 2, 7, 5, 5, 7, 3, 4, 8,
    ];
    let ends = [[4, 0], [0, 4], [8, 4], [4, 8]];
    c.runs(&S.dirt, 0, soil.as_chunks().0);
    c.solid(&S.mossy, [3, 0, 3], [5, 0, 5]);
    c.place(&S.cobble, 5, 0, 3);
    c.place(&S.water, 4, 0, 7);
    c.walls(&S.path, &S.path, [1, 1, 1], [7, 1, 7]);
    c.fill(&S.path, 3..=5, 1, [0, 8]);
    c.fill(&S.path, [0, 8], 1, 3..=5);
    for [x, z] in ends {
        street_end(c, [x, 2, z], &v.pool("streets"));
    }

    let falling = block("minecraft:water[level=8]");
    c.walls(&S.cobble, &S.cobble, [2, 1, 2], [6, 2, 6]);
    c.each(&S.mossy, &[[5, 1, 2], [6, 1, 4]]);
    c.solid(&falling, [3, 1, 3], [5, 1, 5]);
    c.solid(&S.water, [4, 1, 3], [5, 1, 4]);
    c.solid(&S.water, [3, 2, 3], [5, 2, 5]);
    c.posts(&S.spruce_fence, &[2, 6], &[2, 6], [3, 4]);
    log_roof(c, Gable::new(Z, [2, 6], [2, 6], 5, 2));
    c.solid(&K.log_z, [4, 6, 2], [4, 6, 6]);
    c.fill(&S.spruce_fence, 3..=5, 5, [2, 6]);

    spots(c, &v.pool("decor"), GRASS, &[[0, 1, 0], [8, 1, 7]]);
    spots(c, CATS, GRASS, &[[2, 1, 0], [0, 1, 6]]);
    if v.zombie {
        c.runs(&S.grass, 0, overgrown.as_chunks().0);
        c.scatter(&S.dirt, 1, &ends);
    } else {
        let grass = [
            [1, 1],
            [2, 1],
            [5, 1],
            [1, 2],
            [1, 5],
            [8, 5],
            [1, 6],
            [2, 7],
        ];
        c.scatter(&S.grass, 0, &grass);
        c.place(&S.dirt, 1, 0, 8);
        c.fill(&S.wall_torch, [2, 6], 5, [1, 7]);
        c.bell([4, 5, 4], "ceiling", North);
        villagers(c, v, PATH, &[[1, 1, 1], [0, 1, 3], [8, 1, 5]]);
        c.spot([1, 1, 8], IRON_GOLEM, GRASS);
        open(c, 0..=1, &[[0, 1, 8], [6, 1, 8]]);
    }
}

#[rustfmt::skip]
mod data {
    use super::{Fields, Tag};

    pub const ARMOR_STAND: Fields = &[("AbsorptionAmount", Tag::Float(0.0)), ("Air", Tag::Short(300)), ("DeathTime", Tag::Short(0)), ("Dimension", Tag::Int(0)), ("DisabledSlots", Tag::Int(0)), ("FallFlying", Tag::Byte(0)), ("Fire", Tag::Short(-1)), ("HandItems", Tag::List(&[Tag::Compound(&[]), Tag::Compound(&[])])), ("Health", Tag::Float(20.0)), ("HurtByTimestamp", Tag::Int(0)), ("HurtTime", Tag::Short(0)), ("Invisible", Tag::Byte(0)), ("Invulnerable", Tag::Byte(0)), ("Motion", Tag::List(&[Tag::Double(0.0), Tag::Double(-0.0784000015258789), Tag::Double(0.0)])), ("NoBasePlate", Tag::Byte(0)), ("OnGround", Tag::Byte(1)), ("PortalCooldown", Tag::Int(0)), ("ShowArms", Tag::Byte(0)), ("Small", Tag::Byte(0)), ("attributes", Tag::List(&[Tag::Compound(&[("id", Tag::String("minecraft:max_health")), ("base", Tag::Double(20.0))]), Tag::Compound(&[("id", Tag::String("minecraft:knockback_resistance")), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String("minecraft:movement_speed")), ("base", Tag::Double(0.699999988079071))]), Tag::Compound(&[("id", Tag::String("minecraft:armor")), ("base", Tag::Double(0.0))]), Tag::Compound(&[("id", Tag::String("minecraft:armor_toughness")), ("base", Tag::Double(0.0))])])), ("fall_distance", Tag::Double(0.0)), ("id", Tag::String("minecraft:armor_stand"))];
    pub const ARMOR_STAND_ENTITY: Fields = &[("Pose", Tag::Compound(&[("Head", Tag::List(&[Tag::Float(3.978817), Tag::Float(1.5454245), Tag::Float(0.0)])), ("Body", Tag::List(&[Tag::Float(0.0), Tag::Float(1.4669724), Tag::Float(0.0)]))])), ("UUID", Tag::IntArray(&[1744164673, -892581116, -1275854238, 1984293558])), ("equipment", Tag::Compound(&[("head", Tag::Compound(&[("count", Tag::Int(1)), ("id", Tag::String("minecraft:iron_helmet"))]))])), ("Rotation", Tag::List(&[Tag::Float(45.0), Tag::Float(0.0)])), ("Pos", Tag::List(&[Tag::Double(-28.5), Tag::Double(65.0), Tag::Double(30.5)])), ("ArmorItems", Tag::List(&[Tag::Compound(&[]), Tag::Compound(&[]), Tag::Compound(&[]), Tag::Compound(&[("count", Tag::Int(1)), ("id", Tag::String("minecraft:iron_helmet"))])]))];
    pub const ARMOR_STAND_ENTITY_2: Fields = &[("Pose", Tag::Compound(&[("Head", Tag::List(&[Tag::Float(2.951194), Tag::Float(-8.386092), Tag::Float(0.0)])), ("Body", Tag::List(&[Tag::Float(0.0), Tag::Float(3.02534), Tag::Float(0.0)]))])), ("UUID", Tag::IntArray(&[1243514763, -1783870732, -1260393629, 572257545])), ("equipment", Tag::Compound(&[("chest", Tag::Compound(&[("count", Tag::Int(1)), ("id", Tag::String("minecraft:iron_chestplate"))]))])), ("Rotation", Tag::List(&[Tag::Float(90.0), Tag::Float(0.0)])), ("Pos", Tag::List(&[Tag::Double(-26.5), Tag::Double(65.0), Tag::Double(34.5)])), ("ArmorItems", Tag::List(&[Tag::Compound(&[]), Tag::Compound(&[]), Tag::Compound(&[("count", Tag::Int(1)), ("id", Tag::String("minecraft:iron_chestplate"))]), Tag::Compound(&[])]))];
    pub const CAMPFIRE_DATA: Fields = &[("Items", Tag::List(&[])), ("CookingTimes", Tag::IntArray(&[0, 0, 0, 0])), ("CookingTotalTimes", Tag::IntArray(&[0, 0, 0, 0])), ("id", Tag::String("minecraft:campfire"))];
    pub const SIGN_DATA: Fields = &[("back_text", Tag::Compound(&[("has_glowing_text", Tag::Byte(0)), ("color", Tag::String("black")), ("messages", Tag::List(&[Tag::Compound(&[("text", Tag::String(""))]), Tag::Compound(&[("text", Tag::String(""))]), Tag::Compound(&[("text", Tag::String(""))]), Tag::Compound(&[("text", Tag::String(""))])]))])), ("allow_op_features", Tag::Byte(1)), ("id", Tag::String("minecraft:sign")), ("front_text", Tag::Compound(&[("has_glowing_text", Tag::Byte(0)), ("color", Tag::String("black")), ("messages", Tag::List(&[Tag::Compound(&[("text", Tag::String(""))]), Tag::Compound(&[("text", Tag::String(""))]), Tag::Compound(&[("text", Tag::String(""))]), Tag::Compound(&[("text", Tag::String(""))])]))]))];
}
use data::*;

templates! {
    "taiga" "taiga";
    both {
        "houses/taiga_cartographer_house_1" [7, 10, 8] cartographer_house_1;
        "houses/taiga_fisher_cottage_1" [10, 8, 12] fisher_cottage_1;
        "houses/taiga_large_farm_2" [8, 7, 9] large_farm_2;
        "houses/taiga_library_1" [11, 10, 8] library_1;
        "houses/taiga_medium_house_1" [8, 11, 7] medium_house_1;
        "houses/taiga_medium_house_2" [7, 11, 8] medium_house_2;
        "houses/taiga_medium_house_3" [8, 7, 13] medium_house_3;
        "houses/taiga_medium_house_4" [9, 7, 9] medium_house_4;
        "houses/taiga_shepherds_house_1" [10, 7, 11] shepherds_house_1;
        "houses/taiga_small_house_1" [7, 8, 9] small_house_1;
        "houses/taiga_small_house_2" [7, 7, 7] small_house_2;
        "houses/taiga_small_house_3" [7, 7, 7] small_house_3;
        "houses/taiga_small_house_4" [7, 6, 8] small_house_4;
        "houses/taiga_small_house_5" [9, 7, 7] small_house_5;
        "houses/taiga_temple_1" [13, 14, 11] |c, _| temple_1(c);
        "houses/taiga_tool_smith_1" [11, 6, 8] tool_smith_1;
        "houses/taiga_weaponsmith_2" [6, 5, 7] weaponsmith_2;
        "streets/corner_01" [16, 2, 16] village_plains::corner_01_of;
        "streets/corner_02" [16, 2, 16] corner_02_of;
        "streets/corner_03" [4, 2, 4] village_plains::corner_03_of;
        "streets/crossroad_01" [16, 2, 16] village_plains::crossroad_01_of;
        "streets/crossroad_02" [16, 2, 16] village_plains::crossroad_02_of;
        "streets/crossroad_03" [16, 2, 16] village_plains::crossroad_03_of;
        "streets/crossroad_04" [4, 2, 5] crossroad_04_of;
        "streets/crossroad_05" [5, 2, 5] village_plains::crossroad_05_of;
        "streets/crossroad_06" [5, 2, 5] village_plains::crossroad_06_of;
        "streets/straight_01" [16, 2, 16] village_plains::straight_01_of;
        "streets/straight_02" [16, 2, 16] |c, v| straight_with_houses(c, v, 16, [8, 8], &[2, 13]);
        "streets/straight_03" [13, 2, 11] |c, v| village_plains::straight_with_houses(c, v, 11, [3, 7]);
        "streets/straight_04" [11, 2, 9] straight_04_of;
        "streets/straight_05" [20, 2, 17] |c, v| straight_with_houses(c, v, 17, [7, 10], &[3, 13]);
        "streets/straight_06" [21, 2, 18] village_plains::straight_06_of;
        "streets/turn_01" [18, 2, 8] village_plains::turn_01_of;
        "town_centers/taiga_meeting_point_2" [9, 7, 9] meeting_point_2;
    }
    single {
        "houses/taiga_animal_pen_1" [13, 5, 8] taiga_animal_pen_1;
        "houses/taiga_armorer_2" [7, 8, 7] taiga_armorer_2;
        "houses/taiga_armorer_house_1" [10, 7, 7] taiga_armorer_house_1;
        "houses/taiga_butcher_shop_1" [11, 7, 9] taiga_butcher_shop_1;
        "houses/taiga_fletcher_house_1" [10, 6, 11] taiga_fletcher_house_1;
        "houses/taiga_large_farm_1" [10, 7, 10] taiga_large_farm_1;
        "houses/taiga_masons_house_1" [8, 7, 9] taiga_masons_house_1;
        "houses/taiga_small_farm_1" [7, 7, 8] taiga_small_farm_1;
        "houses/taiga_tannery_1" [9, 6, 9] taiga_tannery_1;
        "houses/taiga_weaponsmith_1" [7, 9, 7] taiga_weaponsmith_1;
        "taiga_decoration_1" [3, 2, 6] taiga_decoration_1;
        "taiga_decoration_2" [2, 2, 3] taiga_decoration_2;
        "taiga_decoration_3" [1, 2, 2] |c| stone_seat(c, &S.cobble_wall);
        "taiga_decoration_4" [1, 2, 2] |c| stone_seat(c, &stairs(COBBLE_STAIRS, South));
        "taiga_decoration_5" [1, 1, 1] taiga_decoration_5;
        "taiga_decoration_6" [3, 2, 3] taiga_decoration_6;
        "taiga_lamp_post_1" [1, 2, 1] taiga_lamp_post_1;
        "town_centers/taiga_meeting_point_1" [22, 3, 18] |c| meeting_point_1(c, LIVING);
        "zombie/town_centers/taiga_meeting_point_1" [22, 6, 18] |c| meeting_point_1(c, ZOMBIE);
    }
}

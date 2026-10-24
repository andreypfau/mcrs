use bevy_math::IVec3;
use mcrs_minecraft_core::{Direction, Mirror, Rotation, StaticResourceLocation, rl};
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_random::{Random, shuffle};
use mcrs_minecraft_worldgen_feature::template::{
    bounding_box, transform, zero_position_with_transform,
};

use super::lowest_corner_site;
use crate::frozen::FrozenStructures;
use crate::piece::{Piece, WoodlandMansionPiece};
use crate::site::{Context, Site, Stub};

/// A mansion template's name, checked when the crate compiles.
macro_rules! mansion {
    ($path:literal) => {
        const {
            match StaticResourceLocation::new_static(concat!("minecraft:woodland_mansion/", $path))
            {
                Ok(name) => name,
                Err(reason) => panic!("{}", reason),
            }
        }
    };
}

pub const TEMPLATES: &[StaticResourceLocation] = &[
    rl!("minecraft:woodland_mansion/1x1_a1"),
    rl!("minecraft:woodland_mansion/1x1_a2"),
    rl!("minecraft:woodland_mansion/1x1_a3"),
    rl!("minecraft:woodland_mansion/1x1_a4"),
    rl!("minecraft:woodland_mansion/1x1_a5"),
    rl!("minecraft:woodland_mansion/1x1_as1"),
    rl!("minecraft:woodland_mansion/1x1_as2"),
    rl!("minecraft:woodland_mansion/1x1_as3"),
    rl!("minecraft:woodland_mansion/1x1_as4"),
    rl!("minecraft:woodland_mansion/1x1_b1"),
    rl!("minecraft:woodland_mansion/1x1_b2"),
    rl!("minecraft:woodland_mansion/1x1_b3"),
    rl!("minecraft:woodland_mansion/1x1_b4"),
    rl!("minecraft:woodland_mansion/1x1_b5"),
    rl!("minecraft:woodland_mansion/1x2_a1"),
    rl!("minecraft:woodland_mansion/1x2_a2"),
    rl!("minecraft:woodland_mansion/1x2_a3"),
    rl!("minecraft:woodland_mansion/1x2_a4"),
    rl!("minecraft:woodland_mansion/1x2_a5"),
    rl!("minecraft:woodland_mansion/1x2_a6"),
    rl!("minecraft:woodland_mansion/1x2_a7"),
    rl!("minecraft:woodland_mansion/1x2_a8"),
    rl!("minecraft:woodland_mansion/1x2_a9"),
    rl!("minecraft:woodland_mansion/1x2_b1"),
    rl!("minecraft:woodland_mansion/1x2_b2"),
    rl!("minecraft:woodland_mansion/1x2_b3"),
    rl!("minecraft:woodland_mansion/1x2_b4"),
    rl!("minecraft:woodland_mansion/1x2_b5"),
    rl!("minecraft:woodland_mansion/1x2_c_stairs"),
    rl!("minecraft:woodland_mansion/1x2_c1"),
    rl!("minecraft:woodland_mansion/1x2_c2"),
    rl!("minecraft:woodland_mansion/1x2_c3"),
    rl!("minecraft:woodland_mansion/1x2_c4"),
    rl!("minecraft:woodland_mansion/1x2_d_stairs"),
    rl!("minecraft:woodland_mansion/1x2_d1"),
    rl!("minecraft:woodland_mansion/1x2_d2"),
    rl!("minecraft:woodland_mansion/1x2_d3"),
    rl!("minecraft:woodland_mansion/1x2_d4"),
    rl!("minecraft:woodland_mansion/1x2_d5"),
    rl!("minecraft:woodland_mansion/1x2_s1"),
    rl!("minecraft:woodland_mansion/1x2_s2"),
    rl!("minecraft:woodland_mansion/1x2_se1"),
    rl!("minecraft:woodland_mansion/2x2_a1"),
    rl!("minecraft:woodland_mansion/2x2_a2"),
    rl!("minecraft:woodland_mansion/2x2_a3"),
    rl!("minecraft:woodland_mansion/2x2_a4"),
    rl!("minecraft:woodland_mansion/2x2_b1"),
    rl!("minecraft:woodland_mansion/2x2_b2"),
    rl!("minecraft:woodland_mansion/2x2_b3"),
    rl!("minecraft:woodland_mansion/2x2_b4"),
    rl!("minecraft:woodland_mansion/2x2_b5"),
    rl!("minecraft:woodland_mansion/2x2_s1"),
    rl!("minecraft:woodland_mansion/carpet_east"),
    rl!("minecraft:woodland_mansion/carpet_north"),
    rl!("minecraft:woodland_mansion/carpet_south_1"),
    rl!("minecraft:woodland_mansion/carpet_south_2"),
    rl!("minecraft:woodland_mansion/carpet_west_1"),
    rl!("minecraft:woodland_mansion/carpet_west_2"),
    rl!("minecraft:woodland_mansion/corridor_floor"),
    rl!("minecraft:woodland_mansion/entrance"),
    rl!("minecraft:woodland_mansion/indoors_door_1"),
    rl!("minecraft:woodland_mansion/indoors_door_2"),
    rl!("minecraft:woodland_mansion/indoors_wall_1"),
    rl!("minecraft:woodland_mansion/indoors_wall_2"),
    rl!("minecraft:woodland_mansion/roof"),
    rl!("minecraft:woodland_mansion/roof_corner"),
    rl!("minecraft:woodland_mansion/roof_front"),
    rl!("minecraft:woodland_mansion/roof_inner_corner"),
    rl!("minecraft:woodland_mansion/small_wall"),
    rl!("minecraft:woodland_mansion/small_wall_corner"),
    rl!("minecraft:woodland_mansion/wall_corner"),
    rl!("minecraft:woodland_mansion/wall_flat"),
    rl!("minecraft:woodland_mansion/wall_window"),
];

pub fn site(ctx: &mut Context<'_>, rng: &mut LegacyRandom) -> Option<(IVec3, Stub)> {
    lowest_corner_site(ctx, rng)
}

pub fn layout(ctx: &mut Context<'_>, mut site: Site) -> Vec<Piece> {
    let Stub::Rotated(rotation) = site.stub else {
        return Vec::new();
    };
    let grid = MansionGrid::new(&mut site.rng);
    let mut placer = Placer {
        frozen: ctx.frozen,
        rng: &mut site.rng,
        pieces: Vec::new(),
        start_x: 0,
        start_y: 0,
    };
    placer.create_mansion(site.position, rotation, &grid);
    placer.pieces
}

const SIZE: usize = 11;
const CLEAR: i32 = 0;
const CORRIDOR: i32 = 1;
const ROOM: i32 = 2;
const START_ROOM: i32 = 3;
const TEST_ROOM: i32 = 4;
const BLOCKED: i32 = 5;
const ROOM_1X1: i32 = 1 << 16;
const ROOM_1X2: i32 = 1 << 17;
const ROOM_2X2: i32 = 1 << 18;
const ROOM_ORIGIN: i32 = 1 << 20;
const ROOM_DOOR: i32 = 1 << 21;
const ROOM_STAIRS: i32 = 1 << 22;
const ROOM_CORRIDOR: i32 = 1 << 23;
const ROOM_TYPE_MASK: i32 = 0xF_0000;
const ROOM_ID_MASK: i32 = 0xFFFF;

/// `Direction.from2DDataValue` of a `nextInt(4)` draw.
fn from_2d_data(value: i32) -> Direction {
    match value {
        0 => Direction::South,
        1 => Direction::West,
        2 => Direction::North,
        _ => Direction::East,
    }
}

fn step(direction: Direction) -> (i32, i32) {
    let normal = direction.normal();
    (normal.x, normal.z)
}

/// `SimpleGrid`: an 11×11 plan whose cells outside read as blocked.
#[derive(Clone)]
struct Grid {
    cells: [[i32; SIZE]; SIZE],
}

impl Grid {
    fn new() -> Self {
        Grid {
            cells: [[CLEAR; SIZE]; SIZE],
        }
    }

    fn get(&self, x: i32, y: i32) -> i32 {
        match (usize::try_from(x), usize::try_from(y)) {
            (Ok(x), Ok(y)) => self
                .cells
                .get(x)
                .and_then(|column| column.get(y))
                .copied()
                .unwrap_or(BLOCKED),
            _ => BLOCKED,
        }
    }

    fn set(&mut self, x: i32, y: i32, value: i32) {
        if let (Ok(x), Ok(y)) = (usize::try_from(x), usize::try_from(y))
            && let Some(cell) = self.cells.get_mut(x).and_then(|column| column.get_mut(y))
        {
            *cell = value;
        }
    }

    fn fill(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, value: i32) {
        for y in y0..=y1 {
            for x in x0..=x1 {
                self.set(x, y, value);
            }
        }
    }

    fn set_if(&mut self, x: i32, y: i32, if_value: i32, value: i32) {
        if self.get(x, y) == if_value {
            self.set(x, y, value);
        }
    }

    fn edges_to(&self, x: i32, y: i32, value: i32) -> bool {
        self.get(x - 1, y) == value
            || self.get(x + 1, y) == value
            || self.get(x, y + 1) == value
            || self.get(x, y - 1) == value
    }

    fn is_house(&self, x: i32, y: i32) -> bool {
        matches!(self.get(x, y), CORRIDOR | ROOM | START_ROOM | TEST_ROOM)
    }
}

struct MansionGrid {
    base: Grid,
    third: Grid,
    rooms: [Grid; 3],
    entrance_x: i32,
    entrance_y: i32,
}

impl MansionGrid {
    fn new(rng: &mut LegacyRandom) -> Self {
        let (ex, ey) = (7, 4);
        let mut base = Grid::new();
        base.fill(ex, ey, ex + 1, ey + 1, START_ROOM);
        base.fill(ex - 1, ey, ex - 1, ey + 1, ROOM);
        base.fill(ex + 2, ey - 2, ex + 3, ey + 3, BLOCKED);
        base.fill(ex + 1, ey - 2, ex + 1, ey - 1, CORRIDOR);
        base.fill(ex + 1, ey + 2, ex + 1, ey + 3, CORRIDOR);
        base.set(ex - 1, ey - 1, CORRIDOR);
        base.set(ex - 1, ey + 2, CORRIDOR);
        base.fill(0, 0, 11, 1, BLOCKED);
        base.fill(0, 9, 11, 11, BLOCKED);
        recursive_corridor(&mut base, rng, ex, ey - 2, Direction::West, 6);
        recursive_corridor(&mut base, rng, ex, ey + 3, Direction::West, 6);
        recursive_corridor(&mut base, rng, ex - 2, ey - 1, Direction::West, 3);
        recursive_corridor(&mut base, rng, ex - 2, ey + 2, Direction::West, 3);
        while clean_edges(&mut base) {}

        let [mut first_rooms, mut second_rooms, mut third_rooms] =
            [Grid::new(), Grid::new(), Grid::new()];
        identify_rooms(&base, &mut first_rooms, rng);
        identify_rooms(&base, &mut second_rooms, rng);
        first_rooms.fill(ex + 1, ey, ex + 1, ey + 1, ROOM_CORRIDOR);
        second_rooms.fill(ex + 1, ey, ex + 1, ey + 1, ROOM_CORRIDOR);
        let mut third = Grid::new();
        setup_third_floor(&base, &mut third, &mut second_rooms, &mut third_rooms, rng);
        identify_rooms(&third, &mut third_rooms, rng);
        MansionGrid {
            base,
            third,
            rooms: [first_rooms, second_rooms, third_rooms],
            entrance_x: ex,
            entrance_y: ey,
        }
    }
}

fn is_room_id(rooms: &Grid, x: i32, y: i32, room_id: i32) -> bool {
    (rooms.get(x, y) & ROOM_ID_MASK) == room_id
}

fn room_1x2_direction(rooms: &Grid, x: i32, y: i32, room_id: i32) -> Option<Direction> {
    Direction::HORIZONTAL.into_iter().find(|direction| {
        let (sx, sz) = step(*direction);
        (rooms.get(x + sx, y + sz) & ROOM_ID_MASK) == room_id
    })
}

fn recursive_corridor(
    grid: &mut Grid,
    rng: &mut LegacyRandom,
    x: i32,
    y: i32,
    heading: Direction,
    depth: i32,
) {
    if depth <= 0 {
        return;
    }
    grid.set(x, y, CORRIDOR);
    let (hx, hz) = step(heading);
    grid.set_if(x + hx, y + hz, CLEAR, CORRIDOR);
    for _ in 0..8 {
        let next = from_2d_data(rng.next_i32_bound(4));
        if next != heading.opposite() && (next != Direction::East || !rng.next_bool()) {
            let (nx, ny) = (x + hx, y + hz);
            let (sx, sz) = step(next);
            if grid.get(nx + sx, ny + sz) == CLEAR && grid.get(nx + sx * 2, ny + sz * 2) == CLEAR {
                recursive_corridor(grid, rng, x + hx + sx, y + hz + sz, next, depth - 1);
                break;
            }
        }
    }
    let (cx, cz) = step(heading.clockwise());
    let (ccx, ccz) = step(Rotation::Counterclockwise90.rotate(heading));
    grid.set_if(x + cx, y + cz, CLEAR, ROOM);
    grid.set_if(x + ccx, y + ccz, CLEAR, ROOM);
    grid.set_if(x + hx + cx, y + hz + cz, CLEAR, ROOM);
    grid.set_if(x + hx + ccx, y + hz + ccz, CLEAR, ROOM);
    grid.set_if(x + hx * 2, y + hz * 2, CLEAR, ROOM);
    grid.set_if(x + cx * 2, y + cz * 2, CLEAR, ROOM);
    grid.set_if(x + ccx * 2, y + ccz * 2, CLEAR, ROOM);
}

fn clean_edges(grid: &mut Grid) -> bool {
    let mut touched = false;
    for y in 0..SIZE as i32 {
        for x in 0..SIZE as i32 {
            if grid.get(x, y) != CLEAR {
                continue;
            }
            let direct = [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)]
                .into_iter()
                .filter(|&(x, y)| grid.is_house(x, y))
                .count();
            if direct >= 3 {
                grid.set(x, y, ROOM);
                touched = true;
            } else if direct == 2 {
                let diagonal = [
                    (x + 1, y + 1),
                    (x - 1, y + 1),
                    (x + 1, y - 1),
                    (x - 1, y - 1),
                ]
                .into_iter()
                .filter(|&(x, y)| grid.is_house(x, y))
                .count();
                if diagonal <= 1 {
                    grid.set(x, y, ROOM);
                    touched = true;
                }
            }
        }
    }
    touched
}

fn setup_third_floor(
    base: &Grid,
    third: &mut Grid,
    second_rooms: &mut Grid,
    third_rooms: &mut Grid,
    rng: &mut LegacyRandom,
) {
    let mut potential_rooms = Vec::new();
    for y in 0..SIZE as i32 {
        for x in 0..SIZE as i32 {
            let room_data = second_rooms.get(x, y);
            if room_data & ROOM_TYPE_MASK == ROOM_1X2 && room_data & ROOM_DOOR == ROOM_DOOR {
                potential_rooms.push((x, y));
            }
        }
    }
    if potential_rooms.is_empty() {
        third.fill(0, 0, SIZE as i32, SIZE as i32, BLOCKED);
        return;
    }
    let Some(&(room_x, room_y)) =
        potential_rooms.get(rng.next_i32_bound(potential_rooms.len() as i32) as usize)
    else {
        return;
    };
    let room_data = second_rooms.get(room_x, room_y);
    second_rooms.set(room_x, room_y, room_data | ROOM_STAIRS);
    let Some(room_dir) = room_1x2_direction(second_rooms, room_x, room_y, room_data & ROOM_ID_MASK)
    else {
        return;
    };
    let (sx, sz) = step(room_dir);
    let (end_x, end_y) = (room_x + sx, room_y + sz);
    for y in 0..SIZE as i32 {
        for x in 0..SIZE as i32 {
            if !base.is_house(x, y) {
                third.set(x, y, BLOCKED);
            } else if x == room_x && y == room_y {
                third.set(x, y, START_ROOM);
            } else if x == end_x && y == end_y {
                third.set(x, y, START_ROOM);
                third_rooms.set(x, y, ROOM_CORRIDOR);
            }
        }
    }
    let potential_corridors: Vec<Direction> = Direction::HORIZONTAL
        .into_iter()
        .filter(|direction| {
            let (sx, sz) = step(*direction);
            third.get(end_x + sx, end_y + sz) == CLEAR
        })
        .collect();
    if potential_corridors.is_empty() {
        third.fill(0, 0, SIZE as i32, SIZE as i32, BLOCKED);
        second_rooms.set(room_x, room_y, room_data);
        return;
    }
    let Some(&corridor_dir) =
        potential_corridors.get(rng.next_i32_bound(potential_corridors.len() as i32) as usize)
    else {
        return;
    };
    let (sx, sz) = step(corridor_dir);
    recursive_corridor(third, rng, end_x + sx, end_y + sz, corridor_dir, 4);
    while clean_edges(third) {}
}

fn identify_rooms(from: &Grid, rooms: &mut Grid, rng: &mut LegacyRandom) {
    let mut positions = Vec::new();
    for y in 0..SIZE as i32 {
        for x in 0..SIZE as i32 {
            if from.get(x, y) == ROOM {
                positions.push((x, y));
            }
        }
    }
    shuffle(&mut positions, rng);
    let mut room_id = 10;
    for (x, y) in positions {
        if rooms.get(x, y) != CLEAR {
            continue;
        }
        let (mut x0, mut x1, mut y0, mut y1) = (x, x, y, y);
        let mut room_type = ROOM_1X1;
        let free = |dx: i32, dy: i32| rooms.get(x + dx, y + dy) == CLEAR;
        let room = |dx: i32, dy: i32| from.get(x + dx, y + dy) == ROOM;
        if free(1, 0) && free(0, 1) && free(1, 1) && room(1, 0) && room(0, 1) && room(1, 1) {
            x1 = x + 1;
            y1 = y + 1;
            room_type = ROOM_2X2;
        } else if free(-1, 0)
            && free(0, 1)
            && free(-1, 1)
            && room(-1, 0)
            && room(0, 1)
            && room(-1, 1)
        {
            x0 = x - 1;
            y1 = y + 1;
            room_type = ROOM_2X2;
        } else if free(-1, 0)
            && free(0, -1)
            && free(-1, -1)
            && room(-1, 0)
            && room(0, -1)
            && room(-1, -1)
        {
            x0 = x - 1;
            y0 = y - 1;
            room_type = ROOM_2X2;
        } else if free(1, 0) && room(1, 0) {
            x1 = x + 1;
            room_type = ROOM_1X2;
        } else if free(0, 1) && room(0, 1) {
            y1 = y + 1;
            room_type = ROOM_1X2;
        } else if free(-1, 0) && room(-1, 0) {
            x0 = x - 1;
            room_type = ROOM_1X2;
        } else if free(0, -1) && room(0, -1) {
            y0 = y - 1;
            room_type = ROOM_1X2;
        }

        let mut door_x = if rng.next_bool() { x0 } else { x1 };
        let mut door_y = if rng.next_bool() { y0 } else { y1 };
        let mut door_flag = ROOM_DOOR;
        let flip_x = |door_x: i32| if door_x == x0 { x1 } else { x0 };
        let flip_y = |door_y: i32| if door_y == y0 { y1 } else { y0 };
        if !from.edges_to(door_x, door_y, CORRIDOR) {
            door_x = flip_x(door_x);
            door_y = flip_y(door_y);
            if !from.edges_to(door_x, door_y, CORRIDOR) {
                door_y = flip_y(door_y);
                if !from.edges_to(door_x, door_y, CORRIDOR) {
                    door_x = flip_x(door_x);
                    door_y = flip_y(door_y);
                    if !from.edges_to(door_x, door_y, CORRIDOR) {
                        door_flag = 0;
                        door_x = x0;
                        door_y = y0;
                    }
                }
            }
        }

        for ry in y0..=y1 {
            for rx in x0..=x1 {
                if rx == door_x && ry == door_y {
                    rooms.set(rx, ry, ROOM_ORIGIN | door_flag | room_type | room_id);
                } else {
                    rooms.set(rx, ry, room_type | room_id);
                }
            }
        }
        room_id += 1;
    }
}

const ROOMS_1X1_A: &[StaticResourceLocation] = &[
    mansion!("1x1_a1"),
    mansion!("1x1_a2"),
    mansion!("1x1_a3"),
    mansion!("1x1_a4"),
    mansion!("1x1_a5"),
];
const ROOMS_1X1_B: &[StaticResourceLocation] = &[
    mansion!("1x1_b1"),
    mansion!("1x1_b2"),
    mansion!("1x1_b3"),
    mansion!("1x1_b4"),
    mansion!("1x1_b5"),
];
const ROOMS_1X1_AS: &[StaticResourceLocation] = &[
    mansion!("1x1_as1"),
    mansion!("1x1_as2"),
    mansion!("1x1_as3"),
    mansion!("1x1_as4"),
];
const ROOMS_1X2_A: &[StaticResourceLocation] = &[
    mansion!("1x2_a1"),
    mansion!("1x2_a2"),
    mansion!("1x2_a3"),
    mansion!("1x2_a4"),
    mansion!("1x2_a5"),
    mansion!("1x2_a6"),
    mansion!("1x2_a7"),
    mansion!("1x2_a8"),
    mansion!("1x2_a9"),
];
const ROOMS_1X2_C: &[StaticResourceLocation] = &[
    mansion!("1x2_c1"),
    mansion!("1x2_c2"),
    mansion!("1x2_c3"),
    mansion!("1x2_c4"),
];
const ROOMS_1X2_B: &[StaticResourceLocation] = &[
    mansion!("1x2_b1"),
    mansion!("1x2_b2"),
    mansion!("1x2_b3"),
    mansion!("1x2_b4"),
    mansion!("1x2_b5"),
];
const ROOMS_1X2_D: &[StaticResourceLocation] = &[
    mansion!("1x2_d1"),
    mansion!("1x2_d2"),
    mansion!("1x2_d3"),
    mansion!("1x2_d4"),
    mansion!("1x2_d5"),
];
const ROOMS_1X2_S: &[StaticResourceLocation] = &[mansion!("1x2_s1"), mansion!("1x2_s2")];
const ROOMS_1X2_SE: &[StaticResourceLocation] = &[mansion!("1x2_se1")];
const ROOMS_2X2_A: &[StaticResourceLocation] = &[
    mansion!("2x2_a1"),
    mansion!("2x2_a2"),
    mansion!("2x2_a3"),
    mansion!("2x2_a4"),
];
const ROOMS_2X2_B: &[StaticResourceLocation] = &[
    mansion!("2x2_b1"),
    mansion!("2x2_b2"),
    mansion!("2x2_b3"),
    mansion!("2x2_b4"),
    mansion!("2x2_b5"),
];

/// The three floors' room pools; the second and third share one.
#[derive(Clone, Copy, PartialEq, Eq)]
enum FloorRooms {
    First,
    Upper,
}

impl FloorRooms {
    fn of(floor: usize) -> Self {
        if floor == 0 {
            FloorRooms::First
        } else {
            FloorRooms::Upper
        }
    }

    fn numbered(
        names: &[StaticResourceLocation],
        rng: &mut LegacyRandom,
    ) -> Option<StaticResourceLocation> {
        names
            .get(rng.next_i32_bound(names.len() as i32) as usize)
            .copied()
    }

    fn room_1x1(self, rng: &mut LegacyRandom) -> Option<StaticResourceLocation> {
        match self {
            FloorRooms::First => Self::numbered(ROOMS_1X1_A, rng),
            FloorRooms::Upper => Self::numbered(ROOMS_1X1_B, rng),
        }
    }

    fn room_1x1_secret(self, rng: &mut LegacyRandom) -> Option<StaticResourceLocation> {
        Self::numbered(ROOMS_1X1_AS, rng)
    }

    fn room_1x2_side_entrance(
        self,
        rng: &mut LegacyRandom,
        stairs: bool,
    ) -> Option<StaticResourceLocation> {
        match self {
            FloorRooms::First => Self::numbered(ROOMS_1X2_A, rng),
            FloorRooms::Upper if stairs => Some(mansion!("1x2_c_stairs")),
            FloorRooms::Upper => Self::numbered(ROOMS_1X2_C, rng),
        }
    }

    fn room_1x2_front_entrance(
        self,
        rng: &mut LegacyRandom,
        stairs: bool,
    ) -> Option<StaticResourceLocation> {
        match self {
            FloorRooms::First => Self::numbered(ROOMS_1X2_B, rng),
            FloorRooms::Upper if stairs => Some(mansion!("1x2_d_stairs")),
            FloorRooms::Upper => Self::numbered(ROOMS_1X2_D, rng),
        }
    }

    fn room_1x2_secret(self, rng: &mut LegacyRandom) -> Option<StaticResourceLocation> {
        match self {
            FloorRooms::First => Self::numbered(ROOMS_1X2_S, rng),
            FloorRooms::Upper => Self::numbered(ROOMS_1X2_SE, rng),
        }
    }

    fn room_2x2(self, rng: &mut LegacyRandom) -> Option<StaticResourceLocation> {
        match self {
            FloorRooms::First => Self::numbered(ROOMS_2X2_A, rng),
            FloorRooms::Upper => Self::numbered(ROOMS_2X2_B, rng),
        }
    }

    fn room_2x2_secret(self) -> StaticResourceLocation {
        mansion!("2x2_s1")
    }
}

struct PlacementData {
    position: IVec3,
    rotation: Rotation,
    wall: StaticResourceLocation,
}

struct Placer<'a> {
    frozen: &'a FrozenStructures,
    rng: &'a mut LegacyRandom,
    pieces: Vec<Piece>,
    start_x: i32,
    start_y: i32,
}

impl Placer<'_> {
    fn add(
        &mut self,
        name: StaticResourceLocation,
        position: IVec3,
        rotation: Rotation,
        mirror: Mirror,
    ) {
        let Some((template, size)) = self.frozen.sized_template(name.as_str()) else {
            return;
        };
        self.pieces
            .push(Piece::WoodlandMansion(WoodlandMansionPiece {
                template,
                position,
                rotation,
                mirror,
                bounds: bounding_box(size, position, rotation, mirror, IVec3::ZERO),
            }));
    }

    #[expect(
        clippy::too_many_lines,
        reason = "places the mansion's walls, roofs and floors in the reference's order, so the pieces come out in the order it lists them"
    )]
    #[expect(
        clippy::excessive_nesting,
        reason = "chooses the door directions of each room and the wall or door piece of each side inside the one pass over the grid, so the pieces keep the reference's order"
    )]
    fn create_mansion(&mut self, origin: IVec3, rotation: Rotation, mansion: &MansionGrid) {
        let mut data = PlacementData {
            position: origin,
            rotation,
            wall: mansion!("wall_flat"),
        };
        self.entrance(&mut data);
        let mut second = PlacementData {
            position: data.position + IVec3::Y * 8,
            rotation: data.rotation,
            wall: mansion!("wall_window"),
        };
        let base = &mansion.base;
        let third = &mansion.third;
        self.start_x = mansion.entrance_x + 1;
        self.start_y = mansion.entrance_y + 1;
        let (end_x, end_y) = (mansion.entrance_x + 1, mansion.entrance_y);
        let (start_x, start_y) = (self.start_x, self.start_y);
        self.traverse_outer_walls(
            &mut data,
            base,
            Direction::South,
            start_x,
            start_y,
            end_x,
            end_y,
        );
        self.traverse_outer_walls(
            &mut second,
            base,
            Direction::South,
            start_x,
            start_y,
            end_x,
            end_y,
        );
        let mut third_data = PlacementData {
            position: data.position + IVec3::Y * 19,
            rotation: data.rotation,
            wall: mansion!("wall_window"),
        };
        'third: for y in 0..SIZE as i32 {
            for x in (0..SIZE as i32).rev() {
                if third.is_house(x, y) {
                    third_data.position = self.grid_pos(third_data.position, rotation, x, y);
                    self.traverse_wall_piece(&mut third_data);
                    self.traverse_outer_walls(&mut third_data, third, Direction::South, x, y, x, y);
                    break 'third;
                }
            }
        }

        self.create_roof(origin + IVec3::Y * 16, rotation, base, Some(third));
        self.create_roof(origin + IVec3::Y * 27, rotation, third, None);

        for (floor, rooms) in mansion.rooms.iter().enumerate() {
            let floor_origin =
                origin + IVec3::Y * (8 * floor as i32 + if floor == 2 { 3 } else { 0 });
            let grid = if floor == 2 { third } else { base };
            let pool = FloorRooms::of(floor);
            let south_piece = if floor == 0 {
                mansion!("carpet_south_1")
            } else {
                mansion!("carpet_south_2")
            };
            let west_piece = if floor == 0 {
                mansion!("carpet_west_1")
            } else {
                mansion!("carpet_west_2")
            };
            let corridor_or_flagged = |x: i32, y: i32| {
                grid.get(x, y) == CORRIDOR || rooms.get(x, y) & ROOM_CORRIDOR == ROOM_CORRIDOR
            };

            for y in 0..SIZE as i32 {
                for x in 0..SIZE as i32 {
                    if grid.get(x, y) != CORRIDOR {
                        continue;
                    }
                    let pos = self.grid_pos(floor_origin, rotation, x, y);
                    self.add(mansion!("corridor_floor"), pos, rotation, Mirror::None);
                    if corridor_or_flagged(x, y - 1) {
                        self.add(
                            mansion!("carpet_north"),
                            rotation.rotate(Direction::East).relative(pos, 1) + IVec3::Y,
                            rotation,
                            Mirror::None,
                        );
                    }
                    if corridor_or_flagged(x + 1, y) {
                        let at = rotation.rotate(Direction::South).relative(pos, 1);
                        let at = rotation.rotate(Direction::East).relative(at, 5) + IVec3::Y;
                        self.add(mansion!("carpet_east"), at, rotation, Mirror::None);
                    }
                    if corridor_or_flagged(x, y + 1) {
                        let at = rotation.rotate(Direction::South).relative(pos, 5);
                        let at = rotation.rotate(Direction::West).relative(at, 1);
                        self.add(south_piece, at, rotation, Mirror::None);
                    }
                    if corridor_or_flagged(x - 1, y) {
                        let at = rotation.rotate(Direction::West).relative(pos, 1);
                        let at = rotation.rotate(Direction::North).relative(at, 1);
                        self.add(west_piece, at, rotation, Mirror::None);
                    }
                }
            }

            let wall_piece = if floor == 0 {
                mansion!("indoors_wall_1")
            } else {
                mansion!("indoors_wall_2")
            };
            let door_piece = if floor == 0 {
                mansion!("indoors_door_1")
            } else {
                mansion!("indoors_door_2")
            };
            for y in 0..SIZE as i32 {
                for x in 0..SIZE as i32 {
                    let mut third_floor_start_room = floor == 2 && grid.get(x, y) == START_ROOM;
                    if grid.get(x, y) != ROOM && !third_floor_start_room {
                        continue;
                    }
                    let room_data = rooms.get(x, y);
                    let room_type = room_data & ROOM_TYPE_MASK;
                    let room_id = room_data & ROOM_ID_MASK;
                    third_floor_start_room =
                        third_floor_start_room && room_data & ROOM_CORRIDOR == ROOM_CORRIDOR;
                    let door_dirs: Vec<Direction> = if room_data & ROOM_DOOR == ROOM_DOOR {
                        Direction::HORIZONTAL
                            .into_iter()
                            .filter(|direction| {
                                let (sx, sz) = step(*direction);
                                grid.get(x + sx, y + sz) == CORRIDOR
                            })
                            .collect()
                    } else {
                        Vec::new()
                    };
                    let door_dir = if !door_dirs.is_empty() {
                        door_dirs
                            .get(self.rng.next_i32_bound(door_dirs.len() as i32) as usize)
                            .copied()
                    } else if room_data & ROOM_ORIGIN == ROOM_ORIGIN {
                        Some(Direction::Up)
                    } else {
                        None
                    };

                    let room_pos = rotation
                        .rotate(Direction::South)
                        .relative(floor_origin, 8 + (y - self.start_y) * 8);
                    let room_pos = rotation
                        .rotate(Direction::East)
                        .relative(room_pos, -1 + (x - self.start_x) * 8);
                    let wall_or_door = |side: Direction| {
                        if door_dir == Some(side) {
                            door_piece
                        } else {
                            wall_piece
                        }
                    };
                    if grid.is_house(x - 1, y) && !is_room_id(rooms, x - 1, y, room_id) {
                        self.add(
                            wall_or_door(Direction::West),
                            room_pos,
                            rotation,
                            Mirror::None,
                        );
                    }
                    if grid.get(x + 1, y) == CORRIDOR && !third_floor_start_room {
                        let at = rotation.rotate(Direction::East).relative(room_pos, 8);
                        self.add(wall_or_door(Direction::East), at, rotation, Mirror::None);
                    }
                    if grid.is_house(x, y + 1) && !is_room_id(rooms, x, y + 1, room_id) {
                        let at = rotation.rotate(Direction::South).relative(room_pos, 7);
                        let at = rotation.rotate(Direction::East).relative(at, 7);
                        self.add(
                            wall_or_door(Direction::South),
                            at,
                            rotation.rotated(Rotation::Clockwise90),
                            Mirror::None,
                        );
                    }
                    if grid.get(x, y - 1) == CORRIDOR && !third_floor_start_room {
                        let at = rotation.rotate(Direction::North).relative(room_pos, 1);
                        let at = rotation.rotate(Direction::East).relative(at, 7);
                        self.add(
                            wall_or_door(Direction::North),
                            at,
                            rotation.rotated(Rotation::Clockwise90),
                            Mirror::None,
                        );
                    }

                    match (room_type, door_dir) {
                        (ROOM_1X1, _) => self.add_room_1x1(room_pos, rotation, door_dir, pool),
                        (ROOM_1X2, Some(door_dir)) => {
                            let room_dir = room_1x2_direction(rooms, x, y, room_id);
                            let stairs = room_data & ROOM_STAIRS == ROOM_STAIRS;
                            self.add_room_1x2(room_pos, rotation, room_dir, door_dir, pool, stairs);
                        }
                        (ROOM_2X2, Some(Direction::Up)) => {
                            self.add_room_2x2_secret(room_pos, rotation, pool);
                        }
                        (ROOM_2X2, Some(door_dir)) => {
                            let mut room_dir = door_dir.clockwise();
                            let (sx, sz) = step(room_dir);
                            if !is_room_id(rooms, x + sx, y + sz, room_id) {
                                room_dir = room_dir.opposite();
                            }
                            self.add_room_2x2(room_pos, rotation, room_dir, door_dir, pool);
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    /// The world position of grid cell `(x, y)` from `origin`.
    fn grid_pos(&self, origin: IVec3, rotation: Rotation, x: i32, y: i32) -> IVec3 {
        let pos = rotation
            .rotate(Direction::South)
            .relative(origin, 8 + (y - self.start_y) * 8);
        rotation
            .rotate(Direction::East)
            .relative(pos, (x - self.start_x) * 8)
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "the placement data, the grid, the heading and the start and end cells of one wall traversal, which the reference passes as separate values"
    )]
    fn traverse_outer_walls(
        &mut self,
        data: &mut PlacementData,
        grid: &Grid,
        mut heading: Direction,
        start_x: i32,
        start_y: i32,
        end_x: i32,
        end_y: i32,
    ) {
        let (mut x, mut y) = (start_x, start_y);
        let start_heading = heading;
        loop {
            let (hx, hz) = step(heading);
            let (ccx, ccz) = step(Rotation::Counterclockwise90.rotate(heading));
            if !grid.is_house(x + hx, y + hz) {
                self.traverse_turn(data);
                heading = heading.clockwise();
                if x != end_x || y != end_y || start_heading != heading {
                    self.traverse_wall_piece(data);
                }
            } else if grid.is_house(x + hx + ccx, y + hz + ccz) {
                self.traverse_inner_turn(data);
                x += hx;
                y += hz;
                heading = Rotation::Counterclockwise90.rotate(heading);
            } else {
                x += hx;
                y += hz;
                if x != end_x || y != end_y || start_heading != heading {
                    self.traverse_wall_piece(data);
                }
            }
            if x == end_x && y == end_y && start_heading == heading {
                break;
            }
        }
    }

    #[expect(
        clippy::too_many_lines,
        reason = "lays the roof of one floor cell by cell with the reference's case for each direction, so each case reads against it"
    )]
    #[expect(
        clippy::excessive_nesting,
        reason = "lays the roof of a floor cell by cell with the reference's neighbour test for each direction, so each case reads against it"
    )]
    fn create_roof(
        &mut self,
        roof_origin: IVec3,
        rotation: Rotation,
        grid: &Grid,
        above: Option<&Grid>,
    ) {
        let is_above = |x: i32, y: i32| above.is_some_and(|above| above.is_house(x, y));
        let east = |pos: IVec3, count: i32| rotation.rotate(Direction::East).relative(pos, count);
        let south = |pos: IVec3, count: i32| rotation.rotate(Direction::South).relative(pos, count);
        let west = |pos: IVec3, count: i32| rotation.rotate(Direction::West).relative(pos, count);
        let north = |pos: IVec3, count: i32| rotation.rotate(Direction::North).relative(pos, count);
        for y in 0..SIZE as i32 {
            for x in 0..SIZE as i32 {
                let position = self.grid_pos(roof_origin, rotation, x, y);
                if !grid.is_house(x, y) || is_above(x, y) {
                    continue;
                }
                self.add(
                    mansion!("roof"),
                    position + IVec3::Y * 3,
                    rotation,
                    Mirror::None,
                );
                if !grid.is_house(x + 1, y) {
                    self.add(
                        mansion!("roof_front"),
                        east(position, 6),
                        rotation,
                        Mirror::None,
                    );
                }
                if !grid.is_house(x - 1, y) {
                    let at = south(east(position, 0), 7);
                    self.add(
                        mansion!("roof_front"),
                        at,
                        rotation.rotated(Rotation::Clockwise180),
                        Mirror::None,
                    );
                }
                if !grid.is_house(x, y - 1) {
                    self.add(
                        mansion!("roof_front"),
                        west(position, 1),
                        rotation.rotated(Rotation::Counterclockwise90),
                        Mirror::None,
                    );
                }
                if !grid.is_house(x, y + 1) {
                    let at = south(east(position, 6), 6);
                    self.add(
                        mansion!("roof_front"),
                        at,
                        rotation.rotated(Rotation::Clockwise90),
                        Mirror::None,
                    );
                }
            }
        }

        if above.is_some() {
            for y in 0..SIZE as i32 {
                for x in 0..SIZE as i32 {
                    let position = self.grid_pos(roof_origin, rotation, x, y);
                    if !grid.is_house(x, y) || !is_above(x, y) {
                        continue;
                    }
                    if !grid.is_house(x + 1, y) {
                        self.add(
                            mansion!("small_wall"),
                            east(position, 7),
                            rotation,
                            Mirror::None,
                        );
                    }
                    if !grid.is_house(x - 1, y) {
                        let at = south(west(position, 1), 6);
                        self.add(
                            mansion!("small_wall"),
                            at,
                            rotation.rotated(Rotation::Clockwise180),
                            Mirror::None,
                        );
                    }
                    if !grid.is_house(x, y - 1) {
                        let at = north(west(position, 0), 1);
                        self.add(
                            mansion!("small_wall"),
                            at,
                            rotation.rotated(Rotation::Counterclockwise90),
                            Mirror::None,
                        );
                    }
                    if !grid.is_house(x, y + 1) {
                        let at = south(east(position, 6), 7);
                        self.add(
                            mansion!("small_wall"),
                            at,
                            rotation.rotated(Rotation::Clockwise90),
                            Mirror::None,
                        );
                    }
                    if !grid.is_house(x + 1, y) {
                        if !grid.is_house(x, y - 1) {
                            let at = north(east(position, 7), 2);
                            self.add(mansion!("small_wall_corner"), at, rotation, Mirror::None);
                        }
                        if !grid.is_house(x, y + 1) {
                            let at = south(east(position, 8), 7);
                            self.add(
                                mansion!("small_wall_corner"),
                                at,
                                rotation.rotated(Rotation::Clockwise90),
                                Mirror::None,
                            );
                        }
                    }
                    if !grid.is_house(x - 1, y) {
                        if !grid.is_house(x, y - 1) {
                            let at = north(west(position, 2), 1);
                            self.add(
                                mansion!("small_wall_corner"),
                                at,
                                rotation.rotated(Rotation::Counterclockwise90),
                                Mirror::None,
                            );
                        }
                        if !grid.is_house(x, y + 1) {
                            let at = south(west(position, 1), 8);
                            self.add(
                                mansion!("small_wall_corner"),
                                at,
                                rotation.rotated(Rotation::Clockwise180),
                                Mirror::None,
                            );
                        }
                    }
                }
            }
        }

        for y in 0..SIZE as i32 {
            for x in 0..SIZE as i32 {
                let position = self.grid_pos(roof_origin, rotation, x, y);
                if !grid.is_house(x, y) || is_above(x, y) {
                    continue;
                }
                if !grid.is_house(x + 1, y) {
                    let p2 = east(position, 6);
                    if !grid.is_house(x, y + 1) {
                        self.add(
                            mansion!("roof_corner"),
                            south(p2, 6),
                            rotation,
                            Mirror::None,
                        );
                    } else if grid.is_house(x + 1, y + 1) {
                        self.add(
                            mansion!("roof_inner_corner"),
                            south(p2, 5),
                            rotation,
                            Mirror::None,
                        );
                    }
                    if !grid.is_house(x, y - 1) {
                        self.add(
                            mansion!("roof_corner"),
                            p2,
                            rotation.rotated(Rotation::Counterclockwise90),
                            Mirror::None,
                        );
                    } else if grid.is_house(x + 1, y - 1) {
                        let at = north(east(position, 9), 2);
                        self.add(
                            mansion!("roof_inner_corner"),
                            at,
                            rotation.rotated(Rotation::Clockwise90),
                            Mirror::None,
                        );
                    }
                }
                if !grid.is_house(x - 1, y) {
                    let p2 = south(east(position, 0), 0);
                    if !grid.is_house(x, y + 1) {
                        self.add(
                            mansion!("roof_corner"),
                            south(p2, 6),
                            rotation.rotated(Rotation::Clockwise90),
                            Mirror::None,
                        );
                    } else if grid.is_house(x - 1, y + 1) {
                        let at = west(south(p2, 8), 3);
                        self.add(
                            mansion!("roof_inner_corner"),
                            at,
                            rotation.rotated(Rotation::Counterclockwise90),
                            Mirror::None,
                        );
                    }
                    if !grid.is_house(x, y - 1) {
                        self.add(
                            mansion!("roof_corner"),
                            p2,
                            rotation.rotated(Rotation::Clockwise180),
                            Mirror::None,
                        );
                    } else if grid.is_house(x - 1, y - 1) {
                        self.add(
                            mansion!("roof_inner_corner"),
                            south(p2, 1),
                            rotation.rotated(Rotation::Clockwise180),
                            Mirror::None,
                        );
                    }
                }
            }
        }
    }

    fn entrance(&mut self, data: &mut PlacementData) {
        let west = data.rotation.rotate(Direction::West);
        self.add(
            mansion!("entrance"),
            west.relative(data.position, 9),
            data.rotation,
            Mirror::None,
        );
        data.position = data
            .rotation
            .rotate(Direction::South)
            .relative(data.position, 16);
    }

    fn traverse_wall_piece(&mut self, data: &mut PlacementData) {
        self.add(
            data.wall,
            data.rotation
                .rotate(Direction::East)
                .relative(data.position, 7),
            data.rotation,
            Mirror::None,
        );
        data.position = data
            .rotation
            .rotate(Direction::South)
            .relative(data.position, 8);
    }

    fn traverse_turn(&mut self, data: &mut PlacementData) {
        data.position = data
            .rotation
            .rotate(Direction::South)
            .relative(data.position, -1);
        self.add(
            mansion!("wall_corner"),
            data.position,
            data.rotation,
            Mirror::None,
        );
        data.position = data
            .rotation
            .rotate(Direction::South)
            .relative(data.position, -7);
        data.position = data
            .rotation
            .rotate(Direction::West)
            .relative(data.position, -6);
        data.rotation = data.rotation.rotated(Rotation::Clockwise90);
    }

    fn traverse_inner_turn(&mut self, data: &mut PlacementData) {
        data.position = data
            .rotation
            .rotate(Direction::South)
            .relative(data.position, 6);
        data.position = data
            .rotation
            .rotate(Direction::East)
            .relative(data.position, 8);
        data.rotation = data.rotation.rotated(Rotation::Counterclockwise90);
    }

    fn add_room_1x1(
        &mut self,
        room_pos: IVec3,
        rotation: Rotation,
        door_dir: Option<Direction>,
        pool: FloorRooms,
    ) {
        let mut piece_rot = Rotation::None;
        let mut name = pool.room_1x1(self.rng);
        match door_dir {
            Some(Direction::East) => {}
            Some(Direction::North) => piece_rot = piece_rot.rotated(Rotation::Counterclockwise90),
            Some(Direction::West) => piece_rot = piece_rot.rotated(Rotation::Clockwise180),
            Some(Direction::South) => piece_rot = piece_rot.rotated(Rotation::Clockwise90),
            _ => name = pool.room_1x1_secret(self.rng),
        }
        let orientation =
            zero_position_with_transform(IVec3::new(1, 0, 0), Mirror::None, piece_rot, 7, 7);
        let piece_rot = piece_rot.rotated(rotation);
        let orientation = transform(orientation, Mirror::None, rotation, IVec3::ZERO);
        let pos = room_pos + IVec3::new(orientation.x, 0, orientation.z);
        if let Some(name) = name {
            self.add(name, pos, piece_rot, Mirror::None);
        }
    }

    fn add_room_1x2(
        &mut self,
        room_pos: IVec3,
        rotation: Rotation,
        room_dir: Option<Direction>,
        door_dir: Direction,
        pool: FloorRooms,
        stairs: bool,
    ) {
        use Direction::{East, North, South, Up, West};
        let east = |count: i32| rotation.rotate(East).relative(room_pos, count);
        let south = |pos: IVec3, count: i32| rotation.rotate(South).relative(pos, count);
        let north = |pos: IVec3, count: i32| rotation.rotate(North).relative(pos, count);
        let west = |count: i32| rotation.rotate(West).relative(room_pos, count);
        let Some(room_dir) = room_dir else {
            return;
        };
        let (name, pos, rot, mirror) = match (door_dir, room_dir) {
            (East, South) => (
                pool.room_1x2_side_entrance(self.rng, stairs),
                east(1),
                rotation,
                Mirror::None,
            ),
            (East, North) => (
                pool.room_1x2_side_entrance(self.rng, stairs),
                south(east(1), 6),
                rotation,
                Mirror::LeftRight,
            ),
            (West, North) => (
                pool.room_1x2_side_entrance(self.rng, stairs),
                south(east(7), 6),
                rotation.rotated(Rotation::Clockwise180),
                Mirror::None,
            ),
            (West, South) => (
                pool.room_1x2_side_entrance(self.rng, stairs),
                east(7),
                rotation,
                Mirror::FrontBack,
            ),
            (South, East) => (
                pool.room_1x2_side_entrance(self.rng, stairs),
                east(1),
                rotation.rotated(Rotation::Clockwise90),
                Mirror::LeftRight,
            ),
            (South, West) => (
                pool.room_1x2_side_entrance(self.rng, stairs),
                east(7),
                rotation.rotated(Rotation::Clockwise90),
                Mirror::None,
            ),
            (North, West) => (
                pool.room_1x2_side_entrance(self.rng, stairs),
                south(east(7), 6),
                rotation.rotated(Rotation::Clockwise90),
                Mirror::FrontBack,
            ),
            (North, East) => (
                pool.room_1x2_side_entrance(self.rng, stairs),
                south(east(1), 6),
                rotation.rotated(Rotation::Counterclockwise90),
                Mirror::None,
            ),
            (South, North) => (
                pool.room_1x2_front_entrance(self.rng, stairs),
                north(east(1), 8),
                rotation,
                Mirror::None,
            ),
            (North, South) => (
                pool.room_1x2_front_entrance(self.rng, stairs),
                south(east(7), 14),
                rotation.rotated(Rotation::Clockwise180),
                Mirror::None,
            ),
            (West, East) => (
                pool.room_1x2_front_entrance(self.rng, stairs),
                east(15),
                rotation.rotated(Rotation::Clockwise90),
                Mirror::None,
            ),
            (East, West) => (
                pool.room_1x2_front_entrance(self.rng, stairs),
                south(west(7), 6),
                rotation.rotated(Rotation::Counterclockwise90),
                Mirror::None,
            ),
            (Up, East) => (
                pool.room_1x2_secret(self.rng),
                east(15),
                rotation.rotated(Rotation::Clockwise90),
                Mirror::None,
            ),
            (Up, South) => (
                pool.room_1x2_secret(self.rng),
                north(east(1), 0),
                rotation,
                Mirror::None,
            ),
            _ => return,
        };
        if let Some(name) = name {
            self.add(name, pos, rot, mirror);
        }
    }

    fn add_room_2x2(
        &mut self,
        room_pos: IVec3,
        rotation: Rotation,
        room_dir: Direction,
        door_dir: Direction,
        pool: FloorRooms,
    ) {
        use Direction::{East, North, South, West};
        let (east, south, rot, mirror) = match (door_dir, room_dir) {
            (East, South) => (-7, 0, rotation, Mirror::None),
            (East, North) => (-7, 6, rotation, Mirror::LeftRight),
            (North, East) => (
                1,
                14,
                rotation.rotated(Rotation::Counterclockwise90),
                Mirror::None,
            ),
            (North, West) => (
                7,
                14,
                rotation.rotated(Rotation::Counterclockwise90),
                Mirror::LeftRight,
            ),
            (South, West) => (7, -8, rotation.rotated(Rotation::Clockwise90), Mirror::None),
            (South, East) => (
                1,
                -8,
                rotation.rotated(Rotation::Clockwise90),
                Mirror::LeftRight,
            ),
            (West, North) => (
                15,
                6,
                rotation.rotated(Rotation::Clockwise180),
                Mirror::None,
            ),
            (West, South) => (15, 0, rotation, Mirror::FrontBack),
            _ => (0, 0, rotation, Mirror::None),
        };
        let pos = rotation.rotate(East).relative(room_pos, east);
        let pos = rotation.rotate(South).relative(pos, south);
        let name = pool.room_2x2(self.rng);
        if let Some(name) = name {
            self.add(name, pos, rot, mirror);
        }
    }

    fn add_room_2x2_secret(&mut self, room_pos: IVec3, rotation: Rotation, pool: FloorRooms) {
        let pos = rotation.rotate(Direction::East).relative(room_pos, 1);
        self.add(pool.room_2x2_secret(), pos, rotation, Mirror::None);
    }
}

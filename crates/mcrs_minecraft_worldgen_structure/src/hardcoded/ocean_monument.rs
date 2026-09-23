use bevy_math::IVec3;
use mcrs_minecraft_core::{BoundingBox, Direction};
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_random::{Random, shuffle};
use mcrs_minecraft_worldgen_feature::placement::HeightmapName;
use mcrs_minecraft_worldgen_feature::placer::BiomeMask;

use super::on_top_of_chunk_centre;
use crate::orient::{Orientation, orient_box, world_pos};
use crate::piece::{MonumentChild, MonumentRoom, MonumentRoomKind, OceanMonumentPiece, Piece};
use crate::site::{Context, Site, Stub};

pub const SITE_IMPLIES_PIECE: Option<bool> = Some(true);

const BIOME_RANGE_CHECK: i32 = 29;

pub fn site(
    surrounding: &BiomeMask,
    ctx: &mut Context<'_>,
    _rng: &mut LegacyRandom,
) -> Option<(IVec3, Stub)> {
    let centre = IVec3::new(
        ctx.chunk.min_block_x() + 9,
        ctx.height.sea_level,
        ctx.chunk.min_block_z() + 9,
    );
    if !ctx
        .world
        .all_biomes_within(centre, BIOME_RANGE_CHECK, surrounding)
    {
        return None;
    }
    on_top_of_chunk_centre(ctx, HeightmapName::OceanFloorWg)
}

pub fn layout(ctx: &mut Context<'_>, mut site: Site) -> Vec<Piece> {
    let rng = &mut site.rng;
    let west = ctx.chunk.min_block_x() - BIOME_RANGE_CHECK;
    let north = ctx.chunk.min_block_z() - BIOME_RANGE_CHECK;
    let orientation = Orientation::random(rng);
    vec![Piece::OceanMonument(building(
        rng,
        west,
        north,
        orientation,
    ))]
}

const DOWN: usize = Direction::Down.id();
const UP: usize = Direction::Up.id();
const NORTH: usize = Direction::North.id();
const SOUTH: usize = Direction::South.id();
const WEST: usize = Direction::West.id();
const EAST: usize = Direction::East.id();

const fn room_index(x: i32, y: i32, z: i32) -> i32 {
    y * 25 + z * 5 + x
}

/// The graph while it is built: the rooms plus what only the build reads,
/// and the shuffled order the fitters walk them in.
struct Graph {
    rooms: Vec<MonumentRoom>,
    claimed: Vec<bool>,
    scan_index: Vec<i32>,
    source: usize,
    core: usize,
    order: Vec<usize>,
}

impl Graph {
    fn connect(&mut self, from: usize, direction: usize, to: usize) {
        self.rooms[from].connections[direction] = Some(to as u8);
        self.rooms[to].connections[OPPOSITE[direction]] = Some(from as u8);
    }

    fn neighbour(&self, room: usize, direction: usize) -> Option<usize> {
        self.rooms[room].connections[direction].map(usize::from)
    }

    fn opening(&self, room: usize, direction: usize) -> bool {
        self.rooms[room].has_opening[direction]
    }

    fn past(&self, room: usize, direction: usize) -> usize {
        self.neighbour(room, direction)
            .expect("an open face leads to a room")
    }

    /// `RoomDefinition.findSource`: a walk over open faces stamped by the scan.
    fn find_source(&mut self, room: usize, scan: i32) -> bool {
        if room == self.source {
            return true;
        }
        self.scan_index[room] = scan;
        for direction in 0..6 {
            if let Some(next) = self.neighbour(room, direction)
                && self.opening(room, direction)
                && self.scan_index[next] != scan
                && self.find_source(next, scan)
            {
                return true;
            }
        }
        false
    }
}

const OPPOSITE: [usize; 6] = [UP, DOWN, SOUTH, NORTH, EAST, WEST];

fn grid_has_room(x: i32, y: i32, z: i32) -> bool {
    match y {
        0 | 1 => (0..5).contains(&x) && (0..4).contains(&z),
        2 => (1..4).contains(&x) && (0..2).contains(&z),
        _ => false,
    }
}

/// `MonumentBuilding.generateRoomGraph`.
fn room_graph(rng: &mut LegacyRandom) -> Graph {
    let mut slot = [None; 75];
    let mut rooms = Vec::new();
    for y in 0..3 {
        for z in 0..5 {
            for x in 0..5 {
                if grid_has_room(x, y, z) {
                    slot[room_index(x, y, z) as usize] = Some(rooms.len());
                    rooms.push(MonumentRoom {
                        index: room_index(x, y, z),
                        has_opening: [false; 6],
                        connections: [None; 6],
                    });
                }
            }
        }
    }
    let grid_count = rooms.len();
    for index in [
        MonumentRoom::ROOF,
        MonumentRoom::LEFT_WING,
        MonumentRoom::RIGHT_WING,
    ] {
        rooms.push(MonumentRoom {
            index,
            has_opening: [false; 6],
            connections: [None; 6],
        });
    }
    let (roof, left_wing, right_wing) = (grid_count, grid_count + 1, grid_count + 2);
    let count = rooms.len();
    let at = |x: i32, y: i32, z: i32| slot[room_index(x, y, z) as usize].expect("a grid room");
    let mut graph = Graph {
        rooms,
        claimed: vec![false; count],
        scan_index: vec![0; count],
        source: at(2, 0, 0),
        core: 0,
        order: (0..grid_count).collect(),
    };
    // The reference wires the z axis backwards: north is the room at z + 1.
    let steps = [
        (IVec3::NEG_Y, DOWN),
        (IVec3::Y, UP),
        (IVec3::NEG_Z, SOUTH),
        (IVec3::Z, NORTH),
        (IVec3::NEG_X, WEST),
        (IVec3::X, EAST),
    ];
    for x in 0..5 {
        for z in 0..5 {
            for y in 0..3 {
                if !grid_has_room(x, y, z) {
                    continue;
                }
                for (step, wired) in steps {
                    let (nx, ny, nz) = (x + step.x, y + step.y, z + step.z);
                    if grid_has_room(nx, ny, nz) {
                        graph.connect(at(x, y, z), wired, at(nx, ny, nz));
                    }
                }
            }
        }
    }
    graph.connect(at(2, 2, 0), UP, roof);
    graph.connect(at(0, 1, 0), SOUTH, left_wing);
    graph.connect(at(4, 1, 0), SOUTH, right_wing);
    for special in [roof, left_wing, right_wing] {
        graph.claimed[special] = true;
    }
    let core = at(rng.next_i32_bound(4), 0, 2);
    graph.core = core;
    let core_east = graph.past(core, EAST);
    let core_north = graph.past(core, NORTH);
    let core_east_north = graph.past(core_east, NORTH);
    for room in [
        core,
        core_east,
        core_north,
        core_east_north,
        graph.past(core, UP),
        graph.past(core_east, UP),
        graph.past(core_north, UP),
        graph.past(core_east_north, UP),
    ] {
        graph.claimed[room] = true;
    }
    for room in (0..grid_count).chain([roof]) {
        graph.rooms[room].has_opening = graph.rooms[room].connections.map(|next| next.is_some());
    }
    shuffle(&mut graph.order, rng);
    let mut scan = 1;
    for position in 0..graph.order.len() {
        let room = graph.order[position];
        let mut closed = 0;
        let mut attempts = 0;
        while closed < 2 && attempts < 5 {
            attempts += 1;
            let face = rng.next_i32_bound(6) as usize;
            if !graph.opening(room, face) {
                continue;
            }
            let other = graph.past(room, face);
            let back = OPPOSITE[face];
            graph.rooms[room].has_opening[face] = false;
            graph.rooms[other].has_opening[back] = false;
            let mut reachable = graph.find_source(room, scan);
            scan += 1;
            if reachable {
                reachable = graph.find_source(other, scan);
                scan += 1;
            }
            if reachable {
                closed += 1;
            } else {
                graph.rooms[room].has_opening[face] = true;
                graph.rooms[other].has_opening[back] = true;
            }
        }
    }
    graph.order.extend([roof, left_wing, right_wing]);
    graph
}

#[derive(Clone, Copy)]
enum Fitter {
    DoubleXY,
    DoubleYZ,
    DoubleZ,
    DoubleX,
    DoubleY,
    SimpleTop,
    Simple,
}

const FITTERS: [Fitter; 7] = [
    Fitter::DoubleXY,
    Fitter::DoubleYZ,
    Fitter::DoubleZ,
    Fitter::DoubleX,
    Fitter::DoubleY,
    Fitter::SimpleTop,
    Fitter::Simple,
];

impl Fitter {
    fn fits(self, graph: &Graph, room: usize) -> bool {
        let open_unclaimed = |room: usize, direction: usize| {
            graph.opening(room, direction) && !graph.claimed[graph.past(room, direction)]
        };
        match self {
            Fitter::DoubleXY => {
                open_unclaimed(room, EAST)
                    && open_unclaimed(room, UP)
                    && open_unclaimed(graph.past(room, EAST), UP)
            }
            Fitter::DoubleYZ => {
                open_unclaimed(room, NORTH)
                    && open_unclaimed(room, UP)
                    && open_unclaimed(graph.past(room, NORTH), UP)
            }
            Fitter::DoubleZ => open_unclaimed(room, NORTH),
            Fitter::DoubleX => open_unclaimed(room, EAST),
            Fitter::DoubleY => open_unclaimed(room, UP),
            Fitter::SimpleTop => [WEST, EAST, NORTH, SOUTH, UP]
                .iter()
                .all(|&direction| !graph.opening(room, direction)),
            Fitter::Simple => true,
        }
    }

    /// Each fitter's `create`: the cells it claims and the piece it builds.
    fn create(self, graph: &mut Graph, room: usize, rng: &mut LegacyRandom) -> MonumentRoomKind {
        let cell = room as u8;
        let (claimed, kind) = match self {
            Fitter::DoubleXY => {
                let east = graph.past(room, EAST);
                (
                    vec![room, east, graph.past(room, UP), graph.past(east, UP)],
                    MonumentRoomKind::DoubleXY { room: cell },
                )
            }
            Fitter::DoubleYZ => {
                let north = graph.past(room, NORTH);
                (
                    vec![room, north, graph.past(room, UP), graph.past(north, UP)],
                    MonumentRoomKind::DoubleYZ { room: cell },
                )
            }
            Fitter::DoubleZ => (
                vec![room, graph.past(room, NORTH)],
                MonumentRoomKind::DoubleZ { room: cell },
            ),
            Fitter::DoubleX => (
                vec![room, graph.past(room, EAST)],
                MonumentRoomKind::DoubleX { room: cell },
            ),
            Fitter::DoubleY => (
                vec![room, graph.past(room, UP)],
                MonumentRoomKind::DoubleY { room: cell },
            ),
            Fitter::SimpleTop => (vec![room], MonumentRoomKind::SimpleTop { room: cell }),
            Fitter::Simple => (
                vec![room],
                MonumentRoomKind::Simple {
                    room: cell,
                    main_design: rng.next_i32_bound(3),
                },
            ),
        };
        for room in claimed {
            graph.claimed[room] = true;
        }
        kind
    }
}

/// `OceanMonumentPiece.makeBoundingBox` over a room cell, in the building's
/// own frame before the offset to the world.
fn room_box(
    orientation: Orientation,
    index: i32,
    width: i32,
    height: i32,
    depth: i32,
) -> BoundingBox {
    let x = index % 5;
    let z = index / 5 % 5;
    let y = index / 25;
    let unmoved = orient_box(orientation, IVec3::ZERO, width * 8, height * 4, depth * 8);
    let delta = match orientation {
        Orientation::North => IVec3::new(x * 8, y * 4, -(z + depth) * 8 + 1),
        Orientation::South => IVec3::new(x * 8, y * 4, z * 8),
        Orientation::West => IVec3::new(-(z + depth) * 8 + 1, y * 4, x * 8),
        Orientation::East => IVec3::new(z * 8, y * 4, x * 8),
    };
    unmoved.moved(delta)
}

fn room_size(kind: MonumentRoomKind) -> (i32, i32, i32) {
    match kind {
        MonumentRoomKind::Core { .. } => (2, 2, 2),
        MonumentRoomKind::DoubleX { .. } => (2, 1, 1),
        MonumentRoomKind::DoubleXY { .. } => (2, 2, 1),
        MonumentRoomKind::DoubleY { .. } => (1, 2, 1),
        MonumentRoomKind::DoubleYZ { .. } => (1, 2, 2),
        MonumentRoomKind::DoubleZ { .. } => (1, 1, 2),
        MonumentRoomKind::Entry { .. }
        | MonumentRoomKind::Simple { .. }
        | MonumentRoomKind::SimpleTop { .. } => (1, 1, 1),
        MonumentRoomKind::Wing { .. } | MonumentRoomKind::Penthouse => {
            unreachable!("wings and the penthouse take their box from the building")
        }
    }
}

/// `MonumentBuilding`'s constructor.
fn building(
    rng: &mut LegacyRandom,
    west: i32,
    north: i32,
    orientation: Orientation,
) -> OceanMonumentPiece {
    let bounds = orient_box(
        orientation,
        IVec3::new(west, OceanMonumentPiece::FLOOR, north),
        OceanMonumentPiece::WIDTH,
        OceanMonumentPiece::HEIGHT,
        OceanMonumentPiece::DEPTH,
    );
    let mut graph = room_graph(rng);
    graph.claimed[graph.source] = true;
    let mut kinds = vec![
        MonumentRoomKind::Entry {
            room: graph.source as u8,
        },
        MonumentRoomKind::Core {
            room: graph.core as u8,
        },
    ];
    for position in 0..graph.order.len() {
        let room = graph.order[position];
        if graph.claimed[room] || graph.rooms[room].is_special() {
            continue;
        }
        if let Some(fitter) = FITTERS.iter().find(|fitter| fitter.fits(&graph, room)) {
            kinds.push(fitter.create(&mut graph, room, rng));
        }
    }
    let offset = *world_pos(Some(orientation), bounds, IVec3::new(9, 0, 22));
    let mut children: Vec<MonumentChild> = kinds
        .into_iter()
        .map(|kind| {
            let (width, height, depth) = room_size(kind);
            let index = graph.rooms[room_of(kind)].index;
            MonumentChild {
                kind,
                bounds: room_box(orientation, index, width, height, depth).moved(offset),
            }
        })
        .collect();
    let corner = |a: [i32; 3], b: [i32; 3]| {
        BoundingBox::from_corners(
            world_pos(Some(orientation), bounds, a.into()),
            world_pos(Some(orientation), bounds, b.into()),
        )
    };
    let wing_random = rng.next_i32();
    for (kind, bounds) in [
        (
            MonumentRoomKind::Wing {
                main_design: wing_random & 1,
            },
            corner([1, 1, 1], [23, 8, 21]),
        ),
        (
            MonumentRoomKind::Wing {
                main_design: wing_random.wrapping_add(1) & 1,
            },
            corner([34, 1, 1], [56, 8, 21]),
        ),
        (
            MonumentRoomKind::Penthouse,
            corner([22, 13, 22], [35, 17, 35]),
        ),
    ] {
        children.push(MonumentChild { kind, bounds });
    }
    OceanMonumentPiece {
        bounds,
        orientation,
        rooms: graph.rooms,
        children,
    }
}

fn room_of(kind: MonumentRoomKind) -> usize {
    match kind {
        MonumentRoomKind::Entry { room }
        | MonumentRoomKind::Core { room }
        | MonumentRoomKind::DoubleX { room }
        | MonumentRoomKind::DoubleXY { room }
        | MonumentRoomKind::DoubleY { room }
        | MonumentRoomKind::DoubleYZ { room }
        | MonumentRoomKind::DoubleZ { room }
        | MonumentRoomKind::Simple { room, .. }
        | MonumentRoomKind::SimpleTop { room } => usize::from(room),
        MonumentRoomKind::Wing { .. } | MonumentRoomKind::Penthouse => {
            unreachable!("wings and the penthouse grow from no cell")
        }
    }
}

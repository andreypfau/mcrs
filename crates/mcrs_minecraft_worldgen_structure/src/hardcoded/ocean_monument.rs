use bevy_math::IVec3;
use mcrs_minecraft_block_predicate::predicate::HeightmapName;
use mcrs_minecraft_core::{BoundingBox, Direction};
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_random::{Random, shuffle};
use mcrs_minecraft_worldgen_feature::placer::BiomeMask;

use super::on_top_of_chunk_centre;
use crate::orient::{Orientation, orient_box, world_pos};
use crate::piece::{MonumentChild, MonumentRoom, MonumentRoomKind, OceanMonumentPiece, Piece};
use crate::site::{Context, Site, Stub};

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
    building(rng, west, north, orientation)
        .map(Piece::OceanMonument)
        .into_iter()
        .collect()
}

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
    fn connect(&mut self, from: usize, direction: Direction, to: usize) {
        if let Some(room) = self.rooms.get_mut(from) {
            room.set_connection(direction, to as u8);
        }
        if let Some(room) = self.rooms.get_mut(to) {
            room.set_connection(direction.opposite(), from as u8);
        }
    }

    /// The room past `direction`.
    fn past(&self, room: usize, direction: Direction) -> Option<usize> {
        self.rooms.get(room)?.connection(direction)
    }

    fn opening(&self, room: usize, direction: Direction) -> bool {
        self.rooms
            .get(room)
            .is_some_and(|room| room.opens(direction))
    }

    fn set_opening(&mut self, room: usize, direction: Direction, open: bool) {
        if let Some(room) = self.rooms.get_mut(room) {
            room.set_opening(direction, open);
        }
    }

    fn unclaimed(&self, room: usize) -> bool {
        self.claimed.get(room) == Some(&false)
    }

    fn claim(&mut self, room: usize) {
        if let Some(claimed) = self.claimed.get_mut(room) {
            *claimed = true;
        }
    }

    /// `RoomDefinition.findSource`: a walk over open faces stamped by the scan.
    fn find_source(&mut self, room: usize, scan: i32) -> bool {
        if room == self.source {
            return true;
        }
        if let Some(stamp) = self.scan_index.get_mut(room) {
            *stamp = scan;
        }
        for direction in Direction::all() {
            if let Some(next) = self.past(room, direction)
                && self.opening(room, direction)
                && self.scan_index.get(next) != Some(&scan)
                && self.find_source(next, scan)
            {
                return true;
            }
        }
        false
    }
}

/// `Direction.from3DDataValue` of a `nextInt(6)` draw.
fn from_3d_data(value: i32) -> Direction {
    match value {
        0 => Direction::Down,
        1 => Direction::Up,
        2 => Direction::North,
        3 => Direction::South,
        4 => Direction::West,
        _ => Direction::East,
    }
}

fn grid_has_room(x: i32, y: i32, z: i32) -> bool {
    match y {
        0 | 1 => (0..5).contains(&x) && (0..4).contains(&z),
        2 => (1..4).contains(&x) && (0..2).contains(&z),
        _ => false,
    }
}

/// `MonumentBuilding.generateRoomGraph`.
#[expect(
    clippy::too_many_lines,
    reason = "builds the monument's room graph and draws its openings in the reference's order, so the random draws match it"
)]
fn room_graph(rng: &mut LegacyRandom) -> Option<Graph> {
    let mut slot = [None; 75];
    let mut rooms = Vec::new();
    for y in 0..3 {
        for z in 0..5 {
            for x in 0..5 {
                if grid_has_room(x, y, z)
                    && let Some(slot) = slot.get_mut(room_index(x, y, z) as usize)
                {
                    *slot = Some(rooms.len());
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
    let at = |x: i32, y: i32, z: i32| slot.get(room_index(x, y, z) as usize).copied().flatten();
    let mut graph = Graph {
        rooms,
        claimed: vec![false; count],
        scan_index: vec![0; count],
        source: at(2, 0, 0)?,
        core: 0,
        order: (0..grid_count).collect(),
    };
    // The reference wires the z axis backwards: north is the room at z + 1.
    let steps = [
        (IVec3::NEG_Y, Direction::Down),
        (IVec3::Y, Direction::Up),
        (IVec3::NEG_Z, Direction::South),
        (IVec3::Z, Direction::North),
        (IVec3::NEG_X, Direction::West),
        (IVec3::X, Direction::East),
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
                        graph.connect(at(x, y, z)?, wired, at(nx, ny, nz)?);
                    }
                }
            }
        }
    }
    graph.connect(at(2, 2, 0)?, Direction::Up, roof);
    graph.connect(at(0, 1, 0)?, Direction::South, left_wing);
    graph.connect(at(4, 1, 0)?, Direction::South, right_wing);
    for special in [roof, left_wing, right_wing] {
        graph.claim(special);
    }
    let core = at(rng.next_i32_bound(4), 0, 2)?;
    graph.core = core;
    let core_east = graph.past(core, Direction::East)?;
    let core_north = graph.past(core, Direction::North)?;
    let core_east_north = graph.past(core_east, Direction::North)?;
    for room in [
        core,
        core_east,
        core_north,
        core_east_north,
        graph.past(core, Direction::Up)?,
        graph.past(core_east, Direction::Up)?,
        graph.past(core_north, Direction::Up)?,
        graph.past(core_east_north, Direction::Up)?,
    ] {
        graph.claim(room);
    }
    for room in graph.rooms.iter_mut().take(roof + 1) {
        room.has_opening = room.connections.map(|next| next.is_some());
    }
    shuffle(&mut graph.order, rng);
    let order = std::mem::take(&mut graph.order);
    let mut scan = 1;
    for &room in &order {
        let mut closed = 0;
        let mut attempts = 0;
        while closed < 2 && attempts < 5 {
            attempts += 1;
            let face = from_3d_data(rng.next_i32_bound(6));
            if !graph.opening(room, face) {
                continue;
            }
            let other = graph.past(room, face)?;
            let back = face.opposite();
            graph.set_opening(room, face, false);
            graph.set_opening(other, back, false);
            let mut reachable = graph.find_source(room, scan);
            scan += 1;
            if reachable {
                reachable = graph.find_source(other, scan);
                scan += 1;
            }
            if reachable {
                closed += 1;
            } else {
                graph.set_opening(room, face, true);
                graph.set_opening(other, back, true);
            }
        }
    }
    graph.order = order;
    graph.order.extend([roof, left_wing, right_wing]);
    Some(graph)
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
        use Direction::{East, North, South, Up, West};
        let open_unclaimed = |room: usize, direction: Direction| {
            graph.opening(room, direction)
                && graph
                    .past(room, direction)
                    .is_some_and(|next| graph.unclaimed(next))
        };
        let past = |room: usize, direction: Direction, then: Direction| {
            graph
                .past(room, direction)
                .is_some_and(|next| open_unclaimed(next, then))
        };
        match self {
            Fitter::DoubleXY => {
                open_unclaimed(room, East) && open_unclaimed(room, Up) && past(room, East, Up)
            }
            Fitter::DoubleYZ => {
                open_unclaimed(room, North) && open_unclaimed(room, Up) && past(room, North, Up)
            }
            Fitter::DoubleZ => open_unclaimed(room, North),
            Fitter::DoubleX => open_unclaimed(room, East),
            Fitter::DoubleY => open_unclaimed(room, Up),
            Fitter::SimpleTop => [West, East, North, South, Up]
                .iter()
                .all(|&direction| !graph.opening(room, direction)),
            Fitter::Simple => true,
        }
    }

    /// Each fitter's `create`: the cells it claims and the piece it builds.
    fn create(
        self,
        graph: &mut Graph,
        room: usize,
        rng: &mut LegacyRandom,
    ) -> Option<MonumentRoomKind> {
        use Direction::{East, North, Up};
        let cell = room as u8;
        let (claimed, kind) = match self {
            Fitter::DoubleXY => {
                let east = graph.past(room, East)?;
                (
                    [Some(east), graph.past(room, Up), graph.past(east, Up)],
                    MonumentRoomKind::DoubleXY { room: cell },
                )
            }
            Fitter::DoubleYZ => {
                let north = graph.past(room, North)?;
                (
                    [Some(north), graph.past(room, Up), graph.past(north, Up)],
                    MonumentRoomKind::DoubleYZ { room: cell },
                )
            }
            Fitter::DoubleZ => (
                [graph.past(room, North), None, None],
                MonumentRoomKind::DoubleZ { room: cell },
            ),
            Fitter::DoubleX => (
                [graph.past(room, East), None, None],
                MonumentRoomKind::DoubleX { room: cell },
            ),
            Fitter::DoubleY => (
                [graph.past(room, Up), None, None],
                MonumentRoomKind::DoubleY { room: cell },
            ),
            Fitter::SimpleTop => ([None; 3], MonumentRoomKind::SimpleTop { room: cell }),
            Fitter::Simple => (
                [None; 3],
                MonumentRoomKind::Simple {
                    room: cell,
                    main_design: rng.next_i32_bound(3),
                },
            ),
        };
        graph.claim(room);
        for room in claimed.into_iter().flatten() {
            graph.claim(room);
        }
        Some(kind)
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

/// The cell a room grows from and its size in cells; `None` for the wings
/// and the penthouse, which take their box from the building.
fn room_cells(kind: MonumentRoomKind) -> Option<(usize, (i32, i32, i32))> {
    let (room, size) = match kind {
        MonumentRoomKind::Core { room } => (room, (2, 2, 2)),
        MonumentRoomKind::DoubleX { room } => (room, (2, 1, 1)),
        MonumentRoomKind::DoubleXY { room } => (room, (2, 2, 1)),
        MonumentRoomKind::DoubleY { room } => (room, (1, 2, 1)),
        MonumentRoomKind::DoubleYZ { room } => (room, (1, 2, 2)),
        MonumentRoomKind::DoubleZ { room } => (room, (1, 1, 2)),
        MonumentRoomKind::Entry { room }
        | MonumentRoomKind::Simple { room, .. }
        | MonumentRoomKind::SimpleTop { room } => (room, (1, 1, 1)),
        MonumentRoomKind::Wing { .. } | MonumentRoomKind::Penthouse => return None,
    };
    Some((usize::from(room), size))
}

/// `MonumentBuilding`'s constructor.
fn building(
    rng: &mut LegacyRandom,
    west: i32,
    north: i32,
    orientation: Orientation,
) -> Option<OceanMonumentPiece> {
    let bounds = orient_box(
        orientation,
        IVec3::new(west, OceanMonumentPiece::FLOOR, north),
        OceanMonumentPiece::WIDTH,
        OceanMonumentPiece::HEIGHT,
        OceanMonumentPiece::DEPTH,
    );
    let mut graph = room_graph(rng)?;
    graph.claim(graph.source);
    let mut kinds = vec![
        MonumentRoomKind::Entry {
            room: graph.source as u8,
        },
        MonumentRoomKind::Core {
            room: graph.core as u8,
        },
    ];
    let order = std::mem::take(&mut graph.order);
    for &room in &order {
        if !graph.unclaimed(room) || graph.rooms.get(room).is_none_or(MonumentRoom::is_special) {
            continue;
        }
        if let Some(fitter) = FITTERS.iter().find(|fitter| fitter.fits(&graph, room)) {
            kinds.push(fitter.create(&mut graph, room, rng)?);
        }
    }
    let offset = *world_pos(Some(orientation), bounds, IVec3::new(9, 0, 22));
    let mut children: Vec<MonumentChild> = kinds
        .into_iter()
        .map(|kind| {
            let (room, (width, height, depth)) = room_cells(kind)?;
            let index = graph.rooms.get(room)?.index;
            Some(MonumentChild {
                kind,
                bounds: room_box(orientation, index, width, height, depth).moved(offset),
            })
        })
        .collect::<Option<_>>()?;
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
    Some(OceanMonumentPiece {
        bounds,
        orientation,
        rooms: graph.rooms,
        children,
    })
}

use glam::{IVec3, Vec3};
use mcrs_minecraft_core::{Axis, Direction};

/// What the light of a face reads of the block state in a cell.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct LightState {
    /// `0.2` darkens the corners of the faces beside the state, `1.0` leaves them lit.
    pub occlusion_shade: f32,
    pub solid_render: bool,
    pub light_dampening: u8,
    pub full_collision: bool,
    pub emission: u8,
    pub emissive: bool,
}

impl LightState {
    pub const AIR: LightState = LightState {
        occlusion_shade: 1.0,
        solid_render: false,
        light_dampening: 0,
        full_collision: false,
        emission: 0,
        emissive: false,
    };

    /// Whether a cell of this state lets light reach the edge of a face beside it.
    #[must_use]
    pub fn is_open(&self) -> bool {
        !self.solid_render || self.light_dampening == 0
    }

    /// The light of a cell whose own light is `light`, read for a face of this state.
    #[must_use]
    pub fn light_under(&self, light: CellLight) -> CellLight {
        if self.emissive {
            CellLight::FULL
        } else {
            CellLight {
                block: light.block.max(self.emission),
                sky: light.sky,
            }
        }
    }
}

impl Default for LightState {
    fn default() -> Self {
        Self::AIR
    }
}

/// The block light level and the sky light level of a cell, `0` to `15` each.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct CellLight {
    pub block: u8,
    pub sky: u8,
}

impl CellLight {
    pub const FULL: CellLight = CellLight { block: 15, sky: 15 };

    /// A light volume's form of a cell's light: block light in the high nibble, sky light in
    /// the low.
    #[must_use]
    pub const fn from_packed(packed: u8) -> Self {
        CellLight {
            block: packed >> 4,
            sky: packed & 0x0f,
        }
    }
}

impl From<u8> for CellLight {
    fn from(packed: u8) -> Self {
        Self::from_packed(packed)
    }
}

/// The block light value and the sky light value of a vertex, in sixteenths of a level, `0` to
/// `240` each.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct VertexLight {
    pub block: u8,
    pub sky: u8,
}

impl VertexLight {
    #[must_use]
    pub const fn of_levels(light: CellLight) -> Self {
        VertexLight {
            block: light.block.saturating_mul(16),
            sky: light.sky.saturating_mul(16),
        }
    }

    /// Raises both kinds to at least the light emission of the quad's element, in whole levels.
    #[must_use]
    pub fn with_emission(self, emission: u8) -> Self {
        if emission == 0 {
            return self;
        }
        let raise = |value: u8| (value / 16).max(emission).saturating_mul(16);
        VertexLight {
            block: raise(self.block),
            sky: raise(self.sky),
        }
    }
}

/// What a face reads of one cell: the block state there and the cell's own light.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Sample {
    pub state: LightState,
    pub light: CellLight,
}

/// The occlusion and the light of the four vertices of a quad, in vertex order.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct VertexValues {
    pub occlusion: [f32; 4],
    pub light: [VertexLight; 4],
}

/// The client option that turns smooth light on.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum SmoothLight {
    #[default]
    On,
    Off,
}

/// How the faces of one block state get their occlusion and light.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LightPath {
    Smooth,
    Flat,
}

impl LightPath {
    /// `ambient_occlusion` is the flag of the state's block model, of its first part when it has
    /// more than one.
    #[must_use]
    pub fn choose(option: SmoothLight, own: &LightState, ambient_occlusion: bool) -> Self {
        if option == SmoothLight::On && own.emission == 0 && ambient_occlusion {
            LightPath::Smooth
        } else {
            LightPath::Flat
        }
    }
}

/// The values of the vertices of a quad of the block in the cell at offset zero. `sample`
/// reads the cell at an offset from it, never further than two on any axis.
#[must_use]
pub fn light_quad(
    path: LightPath,
    positions: &[Vec3; 4],
    direction: Direction,
    cull: Option<Direction>,
    sample: impl Fn(IVec3) -> Sample,
) -> VertexValues {
    match path {
        LightPath::Smooth => smooth(positions, direction, sample),
        LightPath::Flat => flat(positions, direction, cull, sample),
    }
}

/// The values of a quad on the flat path: no occlusion, and the light of one cell.
#[must_use]
pub fn flat(
    positions: &[Vec3; 4],
    direction: Direction,
    cull: Option<Direction>,
    sample: impl Fn(IVec3) -> Sample,
) -> VertexValues {
    let own = sample(IVec3::ZERO).state;
    let from = match cull {
        Some(face) => face.normal(),
        None if Bounds::of(positions).on_boundary(direction, own.full_collision) => {
            direction.normal()
        }
        None => IVec3::ZERO,
    };
    let light = VertexLight::of_levels(own.light_under(sample(from).light));
    VertexValues {
        occlusion: [1.0; 4],
        light: [light; 4],
    }
}

/// The values of a quad on the smooth path: each corner blends the cells around it.
#[must_use]
pub fn smooth(
    positions: &[Vec3; 4],
    direction: Direction,
    sample: impl Fn(IVec3) -> Sample,
) -> VertexValues {
    let own = sample(IVec3::ZERO);
    let bounds = Bounds::of(positions);
    let boundary = bounds.on_boundary(direction, own.state.full_collision);
    let normal = direction.normal();
    let base = if boundary { normal } else { IVec3::ZERO };
    let lit = |at: IVec3| {
        let cell = sample(at);
        Lit {
            shade: cell.state.occlusion_shade,
            light: cell.state.light_under(cell.light),
        }
    };

    let edges = edge_directions(direction).map(|edge| {
        let cell = base + edge.normal();
        Edge {
            direction: edge,
            lit: lit(cell),
            open: sample(cell + normal).state.is_open(),
        }
    });
    let front = sample(normal);
    let centre = Lit {
        shade: sample(base).state.occlusion_shade,
        light: if boundary || !front.state.solid_render {
            front.state.light_under(front.light)
        } else {
            own.state.light_under(own.light)
        },
    };

    let [e1, e2, e3, e4] = edges;
    let pairs = [(e1, e4), (e1, e3), (e2, e3), (e2, e4)];
    let corners = pairs.map(|(a, b)| {
        let diagonal = if !a.open && !b.open {
            e1.lit
        } else {
            lit(base + a.direction.normal() + b.direction.normal())
        };
        Corner {
            sides: [a.direction, b.direction],
            values: corner_values(a.lit, b.lit, diagonal, centre),
        }
    });

    let sides = vertex_sides(direction);
    if bounds.is_partial(direction) {
        let values = sides.map(|vertex| interpolate(&corners, bounds.point(vertex), vertex));
        VertexValues {
            occlusion: values.map(|value| value.shade.clamp(0.0, 1.0)),
            light: values.map(|value| value.light),
        }
    } else {
        let values = sides.map(|vertex| {
            corners
                .iter()
                .find(|corner| corner.points_to(vertex))
                .map_or(CornerValues::default(), |corner| corner.values)
        });
        VertexValues {
            occlusion: values.map(|value| value.shade),
            light: values.map(|value| value.light),
        }
    }
}

/// The shade byte of a vertex: its occlusion as a byte, times the face shade of the quad.
#[must_use]
pub fn shade_byte(occlusion: f32, face_shade: f32) -> u8 {
    let occlusion_byte = (occlusion * 255.0).floor();
    (occlusion_byte * face_shade).clamp(0.0, 255.0) as u8
}

/// The vertex positions of the face of the box `min`..`max` that looks along `direction`, in
/// the vertex order that the light of a quad reads.
#[must_use]
pub fn corners(direction: Direction, min: Vec3, max: Vec3) -> [Vec3; 4] {
    let at = |side: Direction, point: Vec3| {
        let value = if side.is_positive() { max } else { min };
        with_component(point, side.axis(), component(value, side.axis()))
    };
    vertex_sides(direction).map(|[first, second]| at(direction, at(second, at(first, Vec3::ZERO))))
}

#[derive(Copy, Clone)]
struct Lit {
    shade: f32,
    light: CellLight,
}

#[derive(Copy, Clone)]
struct Edge {
    direction: Direction,
    lit: Lit,
    open: bool,
}

#[derive(Copy, Clone, Default)]
struct CornerValues {
    shade: f32,
    light: VertexLight,
}

struct Corner {
    sides: [Direction; 2],
    values: CornerValues,
}

impl Corner {
    fn points_to(&self, vertex: [Direction; 2]) -> bool {
        let [first, second] = self.sides;
        vertex == [first, second] || vertex == [second, first]
    }

    /// The side of this corner on the axis of `side`.
    fn side_along(&self, side: Direction) -> Direction {
        let [first, second] = self.sides;
        if first.axis() == side.axis() {
            first
        } else {
            second
        }
    }
}

fn corner_values(a: Lit, b: Lit, diagonal: Lit, centre: Lit) -> CornerValues {
    let shade = (((b.shade + a.shade) + diagonal.shade) + centre.shade) * 0.25;
    let centre_light = centre.light;
    let bright = centre_light.block > 2 || centre_light.sky > 2;
    let neighbour = |light: CellLight| match light {
        _ if !bright => light,
        CellLight { block: 0, sky: 0 } => centre_light,
        CellLight { block, sky: 0 } => CellLight {
            block,
            sky: centre_light.sky,
        },
        _ => light,
    };
    let lights = [
        neighbour(b.light),
        neighbour(a.light),
        neighbour(diagonal.light),
        centre_light,
    ];
    let sum = |level: fn(&CellLight) -> u8| -> u8 {
        let total: u32 = lights.iter().map(|light| u32::from(level(light))).sum();
        u8::try_from(total * 4).unwrap_or(u8::MAX)
    };
    CornerValues {
        shade,
        light: VertexLight {
            block: sum(|light| light.block),
            sky: sum(|light| light.sky),
        },
    }
}

/// The corner values at `point`, a bilinear blend of the four corners in their order.
fn interpolate(corners: &[Corner; 4], point: [f32; 2], vertex: [Direction; 2]) -> CornerValues {
    let [first, second] = vertex;
    let factor = |side: Direction, at: f32| if side.is_positive() { at } else { 1.0 - at };
    let mut shade = 0.0f32;
    let mut block = 0.0f32;
    let mut sky = 0.0f32;
    for corner in corners {
        let weight = factor(corner.side_along(first), point[0])
            * factor(corner.side_along(second), point[1]);
        shade += corner.values.shade * weight;
        block += f32::from(corner.values.light.block) * weight;
        sky += f32::from(corner.values.light.sky) * weight;
    }
    CornerValues {
        shade,
        light: VertexLight {
            block: block as u8,
            sky: sky as u8,
        },
    }
}

const WEST_SOUTH: [Direction; 2] = [Direction::West, Direction::South];
const WEST_NORTH: [Direction; 2] = [Direction::West, Direction::North];
const EAST_NORTH: [Direction; 2] = [Direction::East, Direction::North];
const EAST_SOUTH: [Direction; 2] = [Direction::East, Direction::South];
const EAST_UP: [Direction; 2] = [Direction::East, Direction::Up];
const EAST_DOWN: [Direction; 2] = [Direction::East, Direction::Down];
const WEST_DOWN: [Direction; 2] = [Direction::West, Direction::Down];
const WEST_UP: [Direction; 2] = [Direction::West, Direction::Up];
const UP_NORTH: [Direction; 2] = [Direction::Up, Direction::North];
const DOWN_NORTH: [Direction; 2] = [Direction::Down, Direction::North];
const DOWN_SOUTH: [Direction; 2] = [Direction::Down, Direction::South];
const UP_SOUTH: [Direction; 2] = [Direction::Up, Direction::South];

/// The corner of each vertex index of a quad: its side on each of the two in-plane axes.
const fn vertex_sides(direction: Direction) -> [[Direction; 2]; 4] {
    match direction {
        Direction::Down => [WEST_SOUTH, WEST_NORTH, EAST_NORTH, EAST_SOUTH],
        Direction::Up => [WEST_NORTH, WEST_SOUTH, EAST_SOUTH, EAST_NORTH],
        Direction::North => [EAST_UP, EAST_DOWN, WEST_DOWN, WEST_UP],
        Direction::South => [WEST_UP, WEST_DOWN, EAST_DOWN, EAST_UP],
        Direction::West => [UP_NORTH, DOWN_NORTH, DOWN_SOUTH, UP_SOUTH],
        Direction::East => [UP_SOUTH, DOWN_SOUTH, DOWN_NORTH, UP_NORTH],
    }
}

/// The four edge directions of a face. The first two are opposite, and so are the last two.
const fn edge_directions(direction: Direction) -> [Direction; 4] {
    use Direction::*;
    match direction {
        Down => [West, East, North, South],
        Up => [East, West, North, South],
        North => [Up, Down, East, West],
        South => [West, East, Down, Up],
        West => [Up, Down, North, South],
        East => [Down, Up, North, South],
    }
}

const fn component(point: Vec3, axis: Axis) -> f32 {
    match axis {
        Axis::X => point.x,
        Axis::Y => point.y,
        Axis::Z => point.z,
    }
}

const fn with_component(mut point: Vec3, axis: Axis, value: f32) -> Vec3 {
    match axis {
        Axis::X => point.x = value,
        Axis::Y => point.y = value,
        Axis::Z => point.z = value,
    }
    point
}

const NEAR_MIN: f32 = 0.0001;
const NEAR_MAX: f32 = 0.9999;

struct Bounds {
    min: Vec3,
    max: Vec3,
}

impl Bounds {
    fn of(positions: &[Vec3; 4]) -> Self {
        let [first, rest @ ..] = positions;
        let (min, max) = rest
            .iter()
            .fold((*first, *first), |(min, max), &p| (min.min(p), max.max(p)));
        Bounds { min, max }
    }

    fn on_boundary(&self, direction: Direction, own_full_collision: bool) -> bool {
        let axis = direction.axis();
        let (min, max) = (component(self.min, axis), component(self.max, axis));
        let flush = if direction.is_positive() {
            max > NEAR_MAX
        } else {
            min < NEAR_MIN
        };
        min == max && (flush || own_full_collision)
    }

    fn is_partial(&self, direction: Direction) -> bool {
        vertex_sides(direction)[0].iter().any(|side| {
            let axis = side.axis();
            component(self.min, axis) >= NEAR_MIN || component(self.max, axis) <= NEAR_MAX
        })
    }

    /// The point of a vertex on the two in-plane axes, in the order of its sides.
    fn point(&self, vertex: [Direction; 2]) -> [f32; 2] {
        vertex.map(|side| {
            let value = if side.is_positive() {
                self.max
            } else {
                self.min
            };
            component(value, side.axis())
        })
    }
}

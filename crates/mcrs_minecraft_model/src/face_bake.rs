use bevy::math::{DMat3, DVec3, Vec3};
use mcrs_minecraft_core::Direction;

use crate::block_state_file::Variant;
use crate::model_file::{Axis, Element, ElementTurn, FaceEntry};

/// The turn of a whole model that a blockstate variant gives, in degrees, and its texture lock.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VariantTransform {
    pub x: u16,
    pub y: u16,
    pub z: u16,
    pub uvlock: bool,
}

impl From<&Variant> for VariantTransform {
    fn from(variant: &Variant) -> Self {
        Self {
            x: variant.x,
            y: variant.y,
            z: variant.z,
            uvlock: variant.uvlock,
        }
    }
}

/// One face entry of an element, baked: corners in block space with their sprite coordinates.
#[derive(Clone, Debug, PartialEq)]
pub struct ElementFace {
    pub positions: [Vec3; 4],
    pub uvs: [[f32; 2]; 4],
    pub direction: Direction,
    pub cull: Option<Direction>,
    /// The `texture` string of the face entry.
    pub texture: String,
    pub tint: Option<u32>,
    pub shade_face: Option<Direction>,
    pub light_emission: u8,
}

/// The baked faces of the elements, in element order and, inside an element, in direction
/// order. A face whose corners do not form a box face after the variant turn is an error.
pub fn bake_elements(
    elements: &[Element],
    transform: VariantTransform,
) -> Result<Vec<ElementFace>, String> {
    let turn = QuarterTurn::of(transform);
    let mut faces = Vec::new();
    for element in elements {
        for direction in Direction::all() {
            let Some(entry) = element.face(direction) else {
                continue;
            };
            if draws_face(element, direction) {
                faces.push(bake_face(
                    element,
                    direction,
                    entry,
                    turn,
                    transform.uvlock,
                )?);
            }
        }
    }
    Ok(faces)
}

/// Whether an element draws its face entry of `direction`: each flat axis of the element must
/// be the axis of the direction.
#[must_use]
pub fn draws_face(element: &Element, direction: Direction) -> bool {
    let axis = axis_index(direction);
    let from = element.from.to_array();
    let to = element.to.to_array();
    (0..3).all(|at| at == axis || from.get(at) != to.get(at))
}

fn axis_index(direction: Direction) -> usize {
    match direction {
        Direction::West | Direction::East => 0,
        Direction::Down | Direction::Up => 1,
        Direction::North | Direction::South => 2,
    }
}

fn normal(direction: Direction) -> DVec3 {
    direction.normal().as_dvec3()
}

/// The corners of the `direction` face of the box from `lo` to `hi`: top-left, bottom-left,
/// bottom-right and top-right, seen from outside.
#[must_use]
pub fn box_face_corners(direction: Direction, lo: DVec3, hi: DVec3) -> [DVec3; 4] {
    let (right, up) = face_axes(direction);
    let corner = |top: bool, at_right: bool| {
        let mut point = if direction.is_positive() { hi } else { lo };
        for (axis, at_positive_end) in [(right, at_right), (up, top)] {
            let positive = at_positive_end == axis.is_positive();
            let index = axis_index(axis);
            let value = if positive { hi } else { lo };
            match index {
                0 => point.x = value.x,
                1 => point.y = value.y,
                _ => point.z = value.z,
            }
        }
        point
    };
    [
        corner(true, false),
        corner(false, false),
        corner(false, true),
        corner(true, true),
    ]
}

/// The directions in which the texture of a face goes right and up, seen from outside.
fn face_axes(direction: Direction) -> (Direction, Direction) {
    match direction {
        Direction::Down => (Direction::East, Direction::South),
        Direction::Up => (Direction::East, Direction::North),
        Direction::North => (Direction::West, Direction::Up),
        Direction::South => (Direction::East, Direction::Up),
        Direction::West => (Direction::South, Direction::Up),
        Direction::East => (Direction::North, Direction::Up),
    }
}

fn default_uv(element: &Element, direction: Direction) -> [f64; 4] {
    let (from, to) = (element.from, element.to);
    match direction {
        Direction::Down => [from.x, 16.0 - to.z, to.x, 16.0 - from.z],
        Direction::Up => [from.x, from.z, to.x, to.z],
        Direction::North => [16.0 - to.x, 16.0 - to.y, 16.0 - from.x, 16.0 - from.y],
        Direction::South => [from.x, 16.0 - to.y, to.x, 16.0 - from.y],
        Direction::West => [from.z, 16.0 - to.y, to.z, 16.0 - from.y],
        Direction::East => [16.0 - to.z, 16.0 - to.y, 16.0 - from.z, 16.0 - from.y],
    }
}

fn bake_face(
    element: &Element,
    key: Direction,
    entry: &FaceEntry,
    turn: QuarterTurn,
    uvlock: bool,
) -> Result<ElementFace, String> {
    let mut corners = box_face_corners(key, element.from / 16.0, element.to / 16.0);

    let [u1, v1, u2, v2] = entry.uv.unwrap_or_else(|| default_uv(element, key));
    let uv_corners = [[u1, v1], [u1, v2], [u2, v2], [u2, v1]];
    let mut uvs: [[f64; 2]; 4] = std::array::from_fn(|corner| {
        let [u, v] = uv_corners
            .get((corner + usize::from(entry.rotation)) % 4)
            .copied()
            .unwrap_or_default();
        [u / 16.0, v / 16.0]
    });

    if let Some(rotation) = &element.rotation {
        let matrix = element_matrix(rotation.turn, rotation.rescale);
        let origin = rotation.origin / 16.0;
        for corner in &mut corners {
            *corner = origin + matrix * (*corner - origin);
        }
    }

    let centre = DVec3::splat(0.5);
    for corner in &mut corners {
        *corner = centre + turn.apply(*corner - centre);
    }

    let [p0, p1, p2, _] = corners;
    let cross = (p1 - p0).cross(p2 - p0);
    let flat = !(cross.length() > 0.0 && cross.is_finite());
    let direction = if flat { Direction::Up } else { facing(cross) };

    if element.rotation.is_none() && !flat {
        let lo = corners.into_iter().fold(DVec3::INFINITY, DVec3::min);
        let hi = corners.into_iter().fold(DVec3::NEG_INFINITY, DVec3::max);
        let mut order = [0usize; 4];
        for (slot, expected) in order.iter_mut().zip(box_face_corners(direction, lo, hi)) {
            *slot = corners
                .iter()
                .position(|corner| *corner == expected)
                .ok_or_else(|| format!("a {} face is no box face after its turn", key.name()))?;
        }
        let (old_corners, old_uvs) = (corners, uvs);
        for (to, from) in order.into_iter().enumerate() {
            if let (Some(corner), Some(&moved), Some(uv), Some(&moved_uv)) = (
                corners.get_mut(to),
                old_corners.get(from),
                uvs.get_mut(to),
                old_uvs.get(from),
            ) {
                *corner = moved;
                *uv = moved_uv;
            }
        }
    }

    if uvlock && !turn.is_identity() {
        let turned_key = nearest(turn.apply(normal(key)));
        let frame = face_frame(turned_key).transpose() * turn.matrix() * face_frame(key);
        let inverse = frame.transpose();
        for uv in &mut uvs {
            let [u, v] = *uv;
            let w = inverse * DVec3::new(u - 0.5, v - 0.5, 0.0);
            *uv = [w.x + 0.5, w.y + 0.5];
        }
    }

    Ok(ElementFace {
        positions: corners.map(|corner| corner.as_vec3()),
        uvs: uvs.map(|[u, v]| [u as f32, v as f32]),
        direction,
        cull: entry.cullface.map(|cull| nearest(turn.apply(normal(cull)))),
        texture: entry.texture.clone(),
        tint: entry.tint,
        shade_face: element.shade_direction_override,
        light_emission: element.light_emission,
    })
}

fn element_matrix(turn: ElementTurn, rescale: bool) -> DMat3 {
    let matrix = match turn {
        ElementTurn::Axis { axis, degrees } => {
            let radians = degrees.to_radians();
            match axis {
                Axis::X => DMat3::from_rotation_x(radians),
                Axis::Y => DMat3::from_rotation_y(radians),
                Axis::Z => DMat3::from_rotation_z(radians),
            }
        }
        ElementTurn::Euler(degrees) => {
            DMat3::from_rotation_z(degrees.z.to_radians())
                * DMat3::from_rotation_y(degrees.y.to_radians())
                * DMat3::from_rotation_x(degrees.x.to_radians())
        }
    };
    if !rescale || matrix == DMat3::IDENTITY {
        return matrix;
    }
    let factor = |column: DVec3| 1.0 / column.abs().max_element();
    matrix
        * DMat3::from_diagonal(DVec3::new(
            factor(matrix.x_axis),
            factor(matrix.y_axis),
            factor(matrix.z_axis),
        ))
}

/// The direction of a face with this normal: the largest positive dot product, the first in
/// direction order on a tie, and `up` when no direction has a positive one.
fn facing(normal: DVec3) -> Direction {
    let mut best = Direction::Up;
    let mut best_dot = 0.0;
    for direction in Direction::all() {
        let dot = normal.dot(self::normal(direction));
        if dot > best_dot {
            best = direction;
            best_dot = dot;
        }
    }
    best
}

/// The direction whose normal has the largest dot product with `vector`, the first in direction
/// order on a tie.
#[must_use]
pub fn nearest(vector: DVec3) -> Direction {
    let mut best = Direction::Down;
    let mut best_dot = f64::NEG_INFINITY;
    for direction in Direction::all() {
        let dot = vector.dot(normal(direction));
        if dot > best_dot {
            best = direction;
            best_dot = dot;
        }
    }
    best
}

/// The turn that maps +Z to the normal of the direction.
fn face_frame(direction: Direction) -> DMat3 {
    let rows = |rows: [[f64; 3]; 3]| DMat3::from_cols_array_2d(&rows).transpose();
    match direction {
        Direction::South => DMat3::IDENTITY,
        Direction::East => rows([[0.0, 0.0, 1.0], [0.0, 1.0, 0.0], [-1.0, 0.0, 0.0]]),
        Direction::West => rows([[0.0, 0.0, -1.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]]),
        Direction::North => rows([[-1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, -1.0]]),
        Direction::Up => rows([[1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, -1.0, 0.0]]),
        Direction::Down => rows([[1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]]),
    }
}

/// A product of quarter turns about the axes, with only the entries `0`, `1` and `-1`, so it
/// moves a point with no rounding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct QuarterTurn {
    rows: [[i8; 3]; 3],
}

impl QuarterTurn {
    const IDENTITY: QuarterTurn = QuarterTurn {
        rows: [[1, 0, 0], [0, 1, 0], [0, 0, 1]],
    };
    const X_STEP: QuarterTurn = QuarterTurn {
        rows: [[1, 0, 0], [0, 0, 1], [0, -1, 0]],
    };
    const Y_STEP: QuarterTurn = QuarterTurn {
        rows: [[0, 0, -1], [0, 1, 0], [1, 0, 0]],
    };
    const Z_STEP: QuarterTurn = QuarterTurn {
        rows: [[0, 1, 0], [-1, 0, 0], [0, 0, 1]],
    };

    fn of(transform: VariantTransform) -> Self {
        let mut turn = Self::IDENTITY;
        for (step, degrees) in [
            (Self::X_STEP, transform.x),
            (Self::Y_STEP, transform.y),
            (Self::Z_STEP, transform.z),
        ] {
            for _ in 0..(degrees % 360) / 90 {
                turn = step.then_after(turn);
            }
        }
        turn
    }

    /// `self · first`: `first` applies first.
    fn then_after(self, first: QuarterTurn) -> QuarterTurn {
        let mut rows = [[0i8; 3]; 3];
        for (row, out) in self.rows.iter().zip(&mut rows) {
            for (column, cell) in out.iter_mut().enumerate() {
                *cell = row
                    .iter()
                    .zip(&first.rows)
                    .map(|(a, b)| a * b.get(column).copied().unwrap_or(0))
                    .sum();
            }
        }
        QuarterTurn { rows }
    }

    fn is_identity(self) -> bool {
        self == Self::IDENTITY
    }

    fn apply(self, point: DVec3) -> DVec3 {
        let [x, y, z] = self.rows.map(|row| {
            let [a, b, c] = row.map(f64::from);
            a * point.x + b * point.y + c * point.z
        });
        DVec3::new(x, y, z)
    }

    fn matrix(self) -> DMat3 {
        DMat3::from_cols_array_2d(&self.rows.map(|row| row.map(f64::from))).transpose()
    }
}

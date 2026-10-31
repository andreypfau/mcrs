pub mod block;
pub mod empty;

use glam::Vec3;

use self::block::block_shape;
use self::empty::empty_shape;
use crate::{Axis, Direction};

/// Cells per axis of a face coverage mask. Every occlusion shape in the
/// vanilla corpus is exact at 1/32; 1/16 is not enough for wall-mounted
/// blocks.
pub const FACE_RESOLUTION: usize = 32;

const MASK_WORDS: usize = FACE_RESOLUTION * FACE_RESOLUTION / 64;

pub type FaceMask = [u64; MASK_WORDS];

pub const FACE_MASK_EMPTY: FaceMask = [0; MASK_WORDS];
pub const FACE_MASK_FULL: FaceMask = [u64::MAX; MASK_WORDS];

const R: i32 = FACE_RESOLUTION as i32;
const EPSILON: f32 = 1.0e-6;
const GRID_TOLERANCE: f32 = 1.0e-3;

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

#[derive(Debug)]
pub enum ShapeRepr {
    Empty,
    Block,
    Boxes(Box<[Aabb]>),
}

#[derive(Debug)]
pub struct VoxelShape {
    pub repr: ShapeRepr,
    pub bounds: Aabb,
    pub occludes_full_block: bool,
    face_masks: [FaceMask; 6],
}

impl VoxelShape {
    #[inline]
    pub fn empty() -> &'static VoxelShape {
        empty_shape()
    }

    #[inline]
    pub fn block() -> &'static VoxelShape {
        block_shape()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        matches!(self.repr, ShapeRepr::Empty)
    }

    #[inline]
    pub fn occludes_full_block(&self) -> bool {
        self.occludes_full_block
    }

    /// Coverage of this shape on one face of the unit cube, as a
    /// `FACE_RESOLUTION` square bitmap indexed `v * FACE_RESOLUTION + u` over
    /// the two axes other than the face normal, in ascending axis order.
    #[inline]
    pub fn face_mask(&self, dir: Direction) -> &FaceMask {
        let [down, up, north, south, west, east] = &self.face_masks;
        match dir {
            Direction::Down => down,
            Direction::Up => up,
            Direction::North => north,
            Direction::South => south,
            Direction::West => west,
            Direction::East => east,
        }
    }

    /// Whether the seam between this block and the neighbour on `dir` is
    /// fully covered by the two shapes together. Both arguments are whole
    /// block shapes; the projection onto the shared face happens here.
    pub fn face_occludes(&self, other: &VoxelShape, dir: Direction) -> bool {
        let mine = self.face_mask(dir);
        let theirs = other.face_mask(dir.opposite());
        mine.iter()
            .zip(theirs.iter())
            .all(|(a, b)| a | b == u64::MAX)
    }

    /// Builds a shape from the boxes of a block definition, in the unit-cube
    /// convention. Boxes reaching outside the cube are clipped, matching the
    /// union with the full block that vanilla performs before every occlusion
    /// test.
    pub fn from_boxes(boxes: &[Aabb]) -> VoxelShape {
        let mut off_grid = false;
        let prepared: Vec<Prepared> = boxes
            .iter()
            .filter_map(|b| Prepared::clip(b, &mut off_grid))
            .collect();
        if off_grid {
            tracing::warn!(
                ?boxes,
                "block shape is finer than 1/{FACE_RESOLUTION}; occlusion is rounded to the grid"
            );
        }

        if prepared.is_empty() {
            return VoxelShape {
                repr: ShapeRepr::Empty,
                bounds: Aabb {
                    min: Vec3::ZERO,
                    max: Vec3::ZERO,
                },
                occludes_full_block: false,
                face_masks: [FACE_MASK_EMPTY; 6],
            };
        }

        if fills_unit_cube(&prepared) {
            return VoxelShape {
                repr: ShapeRepr::Block,
                bounds: Aabb {
                    min: Vec3::ZERO,
                    max: Vec3::ONE,
                },
                occludes_full_block: true,
                face_masks: [FACE_MASK_FULL; 6],
            };
        }

        let face_masks = Direction::all().map(|dir| {
            let (axis, positive) = face_axis(dir);
            let (u, v) = tangent_axes(axis);
            let mut mask = FACE_MASK_EMPTY;
            for p in &prepared {
                let reaches = if positive {
                    along(axis, p.hi) >= 1.0 - EPSILON
                } else {
                    along(axis, p.lo) <= EPSILON
                };
                if reaches {
                    fill(
                        &mut mask,
                        along(u, p.cell_lo),
                        along(u, p.cell_hi),
                        along(v, p.cell_lo),
                        along(v, p.cell_hi),
                    );
                }
            }
            mask
        });

        let mut min = Vec3::ONE;
        let mut max = Vec3::ZERO;
        for p in &prepared {
            min = min.min(Vec3::from(p.lo));
            max = max.max(Vec3::from(p.hi));
        }

        VoxelShape {
            repr: ShapeRepr::Boxes(
                prepared
                    .iter()
                    .map(|p| Aabb {
                        min: Vec3::from(p.lo),
                        max: Vec3::from(p.hi),
                    })
                    .collect(),
            ),
            bounds: Aabb { min, max },
            occludes_full_block: false,
            face_masks,
        }
    }
}

struct Prepared {
    lo: [f32; 3],
    hi: [f32; 3],
    cell_lo: [i32; 3],
    cell_hi: [i32; 3],
}

impl Prepared {
    fn clip(b: &Aabb, off_grid: &mut bool) -> Option<Prepared> {
        let lo = b.min.max(Vec3::ZERO);
        let hi = b.max.min(Vec3::ONE);
        if (hi - lo).min_element() < EPSILON {
            return None;
        }
        let mut cell_lo = lo.to_array().map(|c| snap(c, off_grid));
        let mut cell_hi = hi.to_array().map(|c| snap(c, off_grid));
        for (cell_lo, cell_hi) in cell_lo.iter_mut().zip(&mut cell_hi) {
            if *cell_hi <= *cell_lo {
                *cell_lo = (*cell_lo).min(R - 1);
                *cell_hi = *cell_lo + 1;
            }
        }
        Some(Prepared {
            lo: lo.into(),
            hi: hi.into(),
            cell_lo,
            cell_hi,
        })
    }
}

fn snap(coord: f32, off_grid: &mut bool) -> i32 {
    let scaled = coord * R as f32;
    let rounded = scaled.round();
    if (scaled - rounded).abs() > GRID_TOLERANCE {
        *off_grid = true;
    }
    (rounded as i32).clamp(0, R)
}

const fn face_axis(dir: Direction) -> (Axis, bool) {
    (dir.axis(), dir.is_positive())
}

const fn along<T: Copy>(axis: Axis, v: [T; 3]) -> T {
    let [x, y, z] = v;
    match axis {
        Axis::X => x,
        Axis::Y => y,
        Axis::Z => z,
    }
}

const fn tangent_axes(axis: Axis) -> (Axis, Axis) {
    match axis {
        Axis::X => (Axis::Y, Axis::Z),
        Axis::Y => (Axis::X, Axis::Z),
        Axis::Z => (Axis::X, Axis::Y),
    }
}

fn fill(mask: &mut FaceMask, u_lo: i32, u_hi: i32, v_lo: i32, v_hi: i32) {
    for v in v_lo..v_hi {
        for u in u_lo..u_hi {
            let bit = (v * R + u) as usize;
            if let Some(word) = mask.get_mut(bit >> 6) {
                *word |= 1 << (bit & 63);
            }
        }
    }
}

fn fills_unit_cube(prepared: &[Prepared]) -> bool {
    (0..R).all(|y| {
        let mut layer = FACE_MASK_EMPTY;
        for p in prepared {
            if p.cell_lo[1] <= y && y < p.cell_hi[1] {
                fill(
                    &mut layer,
                    p.cell_lo[0],
                    p.cell_hi[0],
                    p.cell_lo[2],
                    p.cell_hi[2],
                );
            }
        }
        layer == FACE_MASK_FULL
    })
}

/// Pool of `&'static VoxelShape` references produced by freeze-time interning.
/// Indices 0 and 1 are reserved for the `Empty` and `Block` singletons.
///
/// `intern` leaks owned shapes via `Box::leak`, paid once at freeze time; the
/// vanilla corpus holds a few hundred distinct occlusion shapes.
#[derive(Default)]
pub struct ShapeRegistry {
    entries: Vec<&'static VoxelShape>,
}

impl ShapeRegistry {
    pub fn new() -> Self {
        Self {
            entries: vec![VoxelShape::empty(), VoxelShape::block()],
        }
    }

    pub fn intern(&mut self, shape: VoxelShape) -> &'static VoxelShape {
        if let Some(existing) = self
            .entries
            .iter()
            .find(|entry| shapes_equal(entry, &shape))
        {
            return existing;
        }
        let leaked: &'static VoxelShape = Box::leak(Box::new(shape));
        self.entries.push(leaked);
        leaked
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True when no user-interned shapes have been added. The two reserved
    /// entries `Empty` and `Block` preloaded by `new` do not count.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.entries.len() <= 2
    }

    #[inline]
    pub fn entries(&self) -> &[&'static VoxelShape] {
        &self.entries
    }
}

fn shapes_equal(a: &VoxelShape, b: &VoxelShape) -> bool {
    if a.bounds != b.bounds
        || a.occludes_full_block != b.occludes_full_block
        || a.face_masks != b.face_masks
    {
        return false;
    }
    match (&a.repr, &b.repr) {
        (ShapeRepr::Empty, ShapeRepr::Empty) => true,
        (ShapeRepr::Block, ShapeRepr::Block) => true,
        (ShapeRepr::Boxes(la), ShapeRepr::Boxes(rb)) => la == rb,
        _ => false,
    }
}

#[cfg(test)]
mod tests;

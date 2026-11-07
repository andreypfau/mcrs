use core::ops::Range;

use mcrs_voxel::BlockPos;

use crate::Aabb;
use crate::Axis;
use crate::CollisionTable;
use crate::ShapeId;
use crate::cell_grid;
use crate::cell_grid::CellGrid;
use crate::grid_lines::BoxLines;
use crate::grid_lines::MAX_BOX_LINES;
use crate::tolerance::COLLISION;

/// A shape that can stop the box of an entity, in world coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Collider {
    /// A shape of the table at a block position.
    Block { shape: ShapeId, at: BlockPos },
    /// The box of an entity, as a shape of one box.
    Entity(Aabb),
    /// All space outside the region of the world border, at all heights.
    Border(BorderRegion),
}

/// The region inside the world border on `x` and `z`, with its edges on whole blocks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BorderRegion {
    pub min_x: f64,
    pub max_x: f64,
    pub min_z: f64,
    pub max_z: f64,
}

impl Collider {
    /// Clips the distance `t` that `mover` moves on `axis` against this collider.
    #[must_use]
    pub fn clip(&self, table: &CollisionTable, axis: Axis, mover: &Aabb, t: f64) -> f64 {
        match self {
            Collider::Block { shape, at } => match table.shape(*shape) {
                Some(shape) => cell_grid::clip(&shape.at(at.as_dvec3()), axis, mover, t),
                None => t,
            },
            Collider::Entity(bounds) => match BoxGrid::of(bounds) {
                Some(grid) => cell_grid::clip(&grid, axis, mover, t),
                None => t,
            },
            Collider::Border(region) => cell_grid::clip(&BorderGrid::of(region), axis, mover, t),
        }
    }

    /// True when the collider and the box overlap with a volume that is more than zero.
    #[must_use]
    pub fn intersects(&self, table: &CollisionTable, query: &Aabb) -> bool {
        match self {
            Collider::Block { shape, at } => table
                .shape(*shape)
                .is_some_and(|shape| cell_grid::intersects(&shape.at(at.as_dvec3()), query)),
            Collider::Entity(bounds) => {
                BoxGrid::of(bounds).is_some_and(|grid| cell_grid::intersects(&grid, query))
            }
            Collider::Border(region) => cell_grid::intersects(&BorderGrid::of(region), query),
        }
    }

    /// Replaces the content of `out` with the `y` grid lines in ascending order.
    pub fn write_y_lines(&self, table: &CollisionTable, out: &mut Vec<f64>) {
        out.clear();
        match self {
            Collider::Block { shape, at } => {
                if let Some(shape) = table.shape(*shape) {
                    let grid = shape.at(at.as_dvec3());
                    out.extend(
                        (0..grid.line_count(Axis::Y)).map(|index| grid.line(Axis::Y, index)),
                    );
                }
            }
            Collider::Entity(bounds) => {
                if let Some(grid) = BoxGrid::of(bounds) {
                    out.extend(
                        (0..grid.line_count(Axis::Y)).map(|index| grid.line(Axis::Y, index)),
                    );
                }
            }
            Collider::Border(_) => out.extend([f64::NEG_INFINITY, f64::INFINITY]),
        }
    }
}

/// The shape of one box in world coordinates.
struct BoxGrid {
    lines: [[f64; MAX_BOX_LINES]; 3],
    counts: [usize; 3],
    cells: [Range<usize>; 3],
}

impl BoxGrid {
    fn of(bounds: &Aabb) -> Option<Self> {
        if bounds.size().min_element() < COLLISION {
            return None;
        }
        let mut lines = [[0.0; MAX_BOX_LINES]; 3];
        let mut counts = [0; 3];
        let mut cells = [0..0, 0..0, 0..0];
        let box_lines = BoxLines::of(bounds);
        for axis in Axis::ALL {
            let (min, max) = (bounds.min_of(axis), bounds.max_of(axis));
            let slot = axis.index();
            let Some(axis_lines) = lines.get_mut(slot) else {
                continue;
            };
            let count = axis.pick(&box_lines).write(axis_lines);
            let mut first = None;
            let mut last = 0;
            for row in 0..count.saturating_sub(1) {
                let start = axis_lines.get(row).copied().unwrap_or(0.0);
                let end = axis_lines.get(row + 1).copied().unwrap_or(0.0);
                let middle = (start + end) / 2.0;
                if middle > min && middle < max {
                    first.get_or_insert(row);
                    last = row + 1;
                }
            }
            if let Some(slot_count) = counts.get_mut(slot) {
                *slot_count = count;
            }
            if let Some(slot_cells) = cells.get_mut(slot) {
                let first = first.unwrap_or(0);
                *slot_cells = first..last.max(first);
            }
        }
        Some(Self {
            lines,
            counts,
            cells,
        })
    }
}

impl CellGrid for BoxGrid {
    fn line_count(&self, axis: Axis) -> usize {
        *axis.pick(&self.counts)
    }

    fn line(&self, axis: Axis, index: usize) -> f64 {
        axis.pick(&self.lines).get(index).copied().unwrap_or(0.0)
    }

    fn any_solid(&self, ranges: [Range<usize>; 3]) -> bool {
        ranges
            .iter()
            .zip(&self.cells)
            .all(|(rows, cells)| rows.start < cells.end && cells.start < rows.end)
    }
}

/// The border region as a shape: three cells on `x` and `z`, one on `y`; every cell except the
/// middle one is solid.
struct BorderGrid {
    x: [f64; 4],
    z: [f64; 4],
}

impl BorderGrid {
    fn of(region: &BorderRegion) -> Self {
        Self {
            x: [f64::NEG_INFINITY, region.min_x, region.max_x, f64::INFINITY],
            z: [f64::NEG_INFINITY, region.min_z, region.max_z, f64::INFINITY],
        }
    }
}

impl CellGrid for BorderGrid {
    fn line_count(&self, axis: Axis) -> usize {
        match axis {
            Axis::Y => 2,
            Axis::X | Axis::Z => 4,
        }
    }

    fn line(&self, axis: Axis, index: usize) -> f64 {
        let lines: &[f64] = match axis {
            Axis::X => &self.x,
            Axis::Y => &[f64::NEG_INFINITY, f64::INFINITY],
            Axis::Z => &self.z,
        };
        lines.get(index).copied().unwrap_or(0.0)
    }

    fn any_solid(&self, [xs, ys, zs]: [Range<usize>; 3]) -> bool {
        let inner_only = |rows: &Range<usize>| rows.start == 1 && rows.end == 2;
        !ys.is_empty() && !xs.is_empty() && !zs.is_empty() && !(inner_only(&xs) && inner_only(&zs))
    }
}

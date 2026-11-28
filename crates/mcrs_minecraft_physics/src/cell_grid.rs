use core::ops::Range;

use crate::Aabb;
use crate::Axis;
use crate::tolerance::COLLISION;

pub(crate) trait CellGrid {
    fn line_count(&self, axis: Axis) -> usize;

    fn line(&self, axis: Axis, index: usize) -> f64;

    fn any_solid(&self, cells: [Range<usize>; 3]) -> bool;
}

pub(crate) fn clip<G: CellGrid>(grid: &G, axis: Axis, mover: &Aabb, t: f64) -> f64 {
    if t.abs() < COLLISION {
        return 0.0;
    }
    let mut cells = [0..0, 0..0, 0..0];
    for other in axis.others() {
        let Some(rows) = cross_section(grid, other, mover) else {
            return t;
        };
        if let Some(slot) = cells.get_mut(other.index()) {
            *slot = rows;
        }
    }
    let layers = grid.line_count(axis).saturating_sub(1);
    if t > 0.0 {
        let front = mover.max_of(axis);
        let reach = front - COLLISION;
        for layer in 0..layers {
            let start = grid.line(axis, layer);
            if start <= reach {
                continue;
            }
            let gap = start - front;
            if gap >= t {
                return t;
            }
            if solid_layer(grid, axis, layer, &cells) {
                return gap;
            }
        }
    } else {
        let back = mover.min_of(axis);
        let reach = back + COLLISION;
        for layer in (0..layers).rev() {
            let end = grid.line(axis, layer + 1);
            if end > reach {
                continue;
            }
            let gap = end - back;
            if gap <= t {
                return t;
            }
            if solid_layer(grid, axis, layer, &cells) {
                return gap;
            }
        }
    }
    t
}

pub(crate) fn intersects<G: CellGrid>(grid: &G, query: &Aabb) -> bool {
    let mut cells = [0..0, 0..0, 0..0];
    for axis in Axis::ALL {
        let low = query.min_of(axis);
        let high = query.max_of(axis);
        let Some(rows) = rows_where(grid, axis, |start, end| start < high && end > low) else {
            return false;
        };
        if let Some(slot) = cells.get_mut(axis.index()) {
            *slot = rows;
        }
    }
    grid.any_solid(cells)
}

fn cross_section<G: CellGrid>(grid: &G, axis: Axis, mover: &Aabb) -> Option<Range<usize>> {
    let high = mover.max_of(axis) - COLLISION;
    let low = mover.min_of(axis) + COLLISION;
    rows_where(grid, axis, |start, end| start <= high && end > low)
}

fn rows_where<G: CellGrid>(
    grid: &G,
    axis: Axis,
    test: impl Fn(f64, f64) -> bool,
) -> Option<Range<usize>> {
    let rows = grid.line_count(axis).saturating_sub(1);
    let mut first = None;
    let mut last = 0;
    for row in 0..rows {
        if test(grid.line(axis, row), grid.line(axis, row + 1)) {
            first.get_or_insert(row);
            last = row + 1;
        }
    }
    first.map(|first| first..last)
}

fn solid_layer<G: CellGrid>(grid: &G, axis: Axis, layer: usize, cells: &[Range<usize>; 3]) -> bool {
    let mut ranges = cells.clone();
    if let Some(slot) = ranges.get_mut(axis.index()) {
        *slot = layer..layer + 1;
    }
    grid.any_solid(ranges)
}

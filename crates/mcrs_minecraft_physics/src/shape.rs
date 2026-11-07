use core::ops::Range;

use glam::DVec3;

use crate::Aabb;
use crate::Axis;
use crate::cell_grid::CellGrid;
use crate::grid_lines::BoxLines;
use crate::grid_lines::MAX_BOX_LINES;
use crate::tolerance::COLLISION;

/// A block shape relative to its block position: the grid lines of each axis and the solid cells
/// between them.
#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
    lines: [Box<[f64]>; 3],
    cells: Cells,
}

#[derive(Clone, Debug, PartialEq)]
enum Cells {
    /// One box: the cells whose indices are in the range on each axis.
    Block([Range<u16>; 3]),
    /// Bit `(x × ny + y) × nz + z` is the cell at `(x, y, z)`.
    Bits(Box<[u64]>),
}

impl Shape {
    /// The shape that the boxes give, or `None` when it has no box with each edge at least
    /// `1.0E-7`.
    #[must_use]
    pub fn from_boxes(boxes: &[Aabb]) -> Option<Self> {
        let kept: Vec<Aabb> = boxes
            .iter()
            .copied()
            .filter(|bounds| bounds.size().min_element() >= COLLISION)
            .collect();
        if kept.is_empty() {
            return None;
        }
        let lines = Axis::ALL.map(|axis| union_lines(&kept, axis));
        let cells =
            match kept.as_slice() {
                [single] => Cells::Block(Axis::ALL.map(|axis| {
                    covered(axis.pick(&lines), single.min_of(axis), single.max_of(axis))
                })),
                _ => Cells::Bits(solid_bits(&lines, &kept)),
            };
        Some(Self { lines, cells })
    }

    /// The grid lines of `axis` in ascending order, relative to the block position.
    #[must_use]
    pub fn lines(&self, axis: Axis) -> &[f64] {
        axis.pick(&self.lines)
    }

    /// True when the shape is one box.
    #[must_use]
    pub fn is_one_box(&self) -> bool {
        matches!(self.cells, Cells::Block(_))
    }

    /// True when the cell is solid. Indices out of the grid are open.
    #[must_use]
    pub fn is_solid(&self, x: usize, y: usize, z: usize) -> bool {
        match &self.cells {
            Cells::Block([rx, ry, rz]) => in_range(rx, x) && in_range(ry, y) && in_range(rz, z),
            Cells::Bits(bits) => {
                let [nx, ny, nz] = self.cell_counts();
                if x >= nx || y >= ny || z >= nz {
                    return false;
                }
                bit(bits, (x * ny + y) * nz + z)
            }
        }
    }

    fn cell_counts(&self) -> [usize; 3] {
        Axis::ALL.map(|axis| self.lines(axis).len().saturating_sub(1))
    }

    pub(crate) fn at(&self, offset: DVec3) -> PlacedShape<'_> {
        PlacedShape {
            shape: self,
            offset,
        }
    }
}

/// A shape moved by a block position.
pub(crate) struct PlacedShape<'a> {
    shape: &'a Shape,
    offset: DVec3,
}

impl CellGrid for PlacedShape<'_> {
    fn line_count(&self, axis: Axis) -> usize {
        self.shape.lines(axis).len()
    }

    fn line(&self, axis: Axis, index: usize) -> f64 {
        self.shape.lines(axis).get(index).copied().unwrap_or(0.0) + axis.of(self.offset)
    }

    fn any_solid(&self, [xs, ys, zs]: [Range<usize>; 3]) -> bool {
        match &self.shape.cells {
            Cells::Block([rx, ry, rz]) => {
                overlaps(rx, &xs) && overlaps(ry, &ys) && overlaps(rz, &zs)
            }
            Cells::Bits(_) => xs.clone().any(|x| {
                ys.clone()
                    .any(|y| zs.clone().any(|z| self.shape.is_solid(x, y, z)))
            }),
        }
    }
}

/// The union of the grid lines of the boxes on one axis; a line joins the last kept line when it is
/// at most `1.0E-7` above it.
fn union_lines(boxes: &[Aabb], axis: Axis) -> Box<[f64]> {
    let mut all = Vec::with_capacity(boxes.len() * MAX_BOX_LINES);
    let mut buffer = [0.0; MAX_BOX_LINES];
    for bounds in boxes {
        let count = axis.pick(&BoxLines::of(bounds)).write(&mut buffer);
        all.extend(buffer.iter().take(count));
    }
    all.sort_by(f64::total_cmp);
    let mut merged: Vec<f64> = Vec::with_capacity(all.len());
    for line in all {
        if merged.last().is_none_or(|&kept| line - COLLISION > kept) {
            merged.push(line);
        }
    }
    merged.into_boxed_slice()
}

/// The cells of `lines` whose middle is inside the interval.
fn covered(lines: &[f64], min: f64, max: f64) -> Range<u16> {
    let mut first = None;
    let mut last = 0;
    for (index, pair) in lines.windows(2).enumerate() {
        if let [start, end] = pair {
            let middle = (start + end) / 2.0;
            if middle > min && middle < max {
                first.get_or_insert(index);
                last = index + 1;
            }
        }
    }
    let first = first.unwrap_or(0);
    to_u16(first)..to_u16(last.max(first))
}

fn solid_bits(lines: &[Box<[f64]>; 3], boxes: &[Aabb]) -> Box<[u64]> {
    let ranges: Vec<[Range<u16>; 3]> = boxes
        .iter()
        .map(|bounds| {
            Axis::ALL
                .map(|axis| covered(axis.pick(lines), bounds.min_of(axis), bounds.max_of(axis)))
        })
        .collect();
    let [nx, ny, nz] = lines.each_ref().map(|axis| axis.len().saturating_sub(1));
    let mut bits = vec![0_u64; (nx * ny * nz).div_ceil(64)];
    for [rx, ry, rz] in &ranges {
        for x in rx.clone() {
            for y in ry.clone() {
                for z in rz.clone() {
                    let index = (usize::from(x) * ny + usize::from(y)) * nz + usize::from(z);
                    if let Some(word) = bits.get_mut(index / 64) {
                        *word |= 1 << (index % 64);
                    }
                }
            }
        }
    }
    bits.into_boxed_slice()
}

fn bit(bits: &[u64], index: usize) -> bool {
    bits.get(index / 64)
        .is_some_and(|word| word >> (index % 64) & 1 == 1)
}

fn in_range(range: &Range<u16>, index: usize) -> bool {
    usize::from(range.start) <= index && index < usize::from(range.end)
}

fn overlaps(cells: &Range<u16>, rows: &Range<usize>) -> bool {
    usize::from(cells.start) < rows.end && rows.start < usize::from(cells.end)
}

fn to_u16(value: usize) -> u16 {
    u16::try_from(value).unwrap_or(u16::MAX)
}

use crate::Aabb;
use crate::Axis;
use crate::tolerance::COLLISION;

/// The largest power of two of the grid of one box: a box face on a grid of `1/2^n` with `n` up
/// to this value gives the grid lines of that grid.
pub(crate) const MAX_GRID_BITS: u32 = 3;

/// The most grid lines that one box gives on one axis.
pub(crate) const MAX_BOX_LINES: usize = (1 << MAX_GRID_BITS) + 1;

/// The grid lines of one box on one axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum BoxLines {
    /// The two faces of the box.
    Faces(f64, f64),
    /// The multiples of `1/2^bits` from 0 to 1.
    Grid(u32),
}

impl BoxLines {
    /// The lines of `bounds` on each axis. When one axis is off the grid of `1/2^MAX_GRID_BITS`,
    /// every axis keeps only the faces.
    pub(crate) fn of(bounds: &Aabb) -> [Self; 3] {
        let bits = Axis::ALL.map(|axis| grid_bits(bounds.min_of(axis), bounds.max_of(axis)));
        match bits {
            [Some(x), Some(y), Some(z)] => {
                [BoxLines::Grid(x), BoxLines::Grid(y), BoxLines::Grid(z)]
            }
            _ => Axis::ALL.map(|axis| BoxLines::Faces(bounds.min_of(axis), bounds.max_of(axis))),
        }
    }

    /// Writes the lines in ascending order into `out` and returns their number.
    pub(crate) fn write(self, out: &mut [f64; MAX_BOX_LINES]) -> usize {
        match self {
            BoxLines::Faces(min, max) => {
                out[0] = min;
                out[1] = max;
                2
            }
            BoxLines::Grid(bits) => {
                let cells = 1_u32 << bits;
                let scale = f64::from(cells);
                for (index, line) in out.iter_mut().enumerate().take(cells as usize + 1) {
                    *line = index as f64 / scale;
                }
                cells as usize + 1
            }
        }
    }
}

fn grid_bits(min: f64, max: f64) -> Option<u32> {
    if min < -COLLISION || max > 1.000_000_1 {
        return None;
    }
    (0..=MAX_GRID_BITS).find(|&bits| {
        let scale = f64::from(1_u32 << bits);
        on_grid(min, scale) && on_grid(max, scale)
    })
}

fn on_grid(value: f64, scale: f64) -> bool {
    let scaled = value * scale;
    (scaled - scaled.round()).abs() < COLLISION * scale
}

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
    /// The lines of the box from `min` to `max` on one axis.
    pub(crate) fn of(min: f64, max: f64) -> Self {
        if min < -COLLISION || max > 1.000_000_1 {
            return BoxLines::Faces(min, max);
        }
        for bits in 0..=MAX_GRID_BITS {
            let scale = f64::from(1_u32 << bits);
            if on_grid(min, scale) && on_grid(max, scale) {
                return BoxLines::Grid(bits);
            }
        }
        BoxLines::Faces(min, max)
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

fn on_grid(value: f64, scale: f64) -> bool {
    let scaled = value * scale;
    (scaled - scaled.round()).abs() < COLLISION * scale
}

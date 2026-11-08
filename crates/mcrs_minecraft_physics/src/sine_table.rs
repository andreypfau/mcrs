use core::f64::consts::PI;

const ENTRIES: usize = 65536;
const INDEX_PER_RADIAN: f64 = 10430.378350470453;
const QUARTER_TURN: f64 = 16384.0;
const DEGREES_TO_RADIANS: f32 = (PI / 180.0) as f32;

/// The client's sine table: 65536 f32 entries over one turn. The input rotation and the teleport
/// rotation read it instead of the exact sine, so the server must read it too.
#[derive(Clone, Debug, PartialEq)]
pub struct SineTable {
    entries: Box<[f32; ENTRIES]>,
}

impl SineTable {
    #[must_use]
    pub fn new() -> Self {
        let mut entries = Box::new([0.0_f32; ENTRIES]);
        for (index, entry) in entries.iter_mut().enumerate() {
            *entry = (index as f64 / INDEX_PER_RADIAN).sin() as f32;
        }
        // The exact zeros and ones at the quarter turns are part of the table, not of the sine.
        for (index, value) in [(0, 0.0), (16384, 1.0), (32768, 0.0), (49152, -1.0)] {
            if let Some(entry) = entries.get_mut(index) {
                *entry = value;
            }
        }
        Self { entries }
    }

    #[must_use]
    pub fn entry(&self, index: u16) -> f32 {
        self.entries.get(usize::from(index)).copied().unwrap_or(0.0)
    }

    /// `sinT` of an f64 angle in radians.
    #[must_use]
    pub fn sin(&self, angle: f64) -> f32 {
        if angle < 0.0 {
            -self.entry(turn_index(-angle * INDEX_PER_RADIAN + 0.5))
        } else {
            self.entry(turn_index(angle * INDEX_PER_RADIAN + 0.5))
        }
    }

    /// `cosT` of an f64 angle in radians.
    #[must_use]
    pub fn cos(&self, angle: f64) -> f32 {
        self.entry(turn_index(
            angle.abs() * INDEX_PER_RADIAN + QUARTER_TURN + 0.5,
        ))
    }
}

impl Default for SineTable {
    fn default() -> Self {
        Self::new()
    }
}

/// The f64 angle in radians of a raw f32 yaw in degrees: the product is an f32 operation.
#[must_use]
pub fn yaw_radians(yaw: f32) -> f64 {
    f64::from(yaw * DEGREES_TO_RADIANS)
}

/// Truncates towards zero, saturating, then keeps the low 16 bits: the index of the table.
fn turn_index(scaled: f64) -> u16 {
    (scaled as i32 & 0xFFFF) as u16
}

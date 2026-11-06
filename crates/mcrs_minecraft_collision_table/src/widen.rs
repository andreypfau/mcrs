/// The steps of the corpus in one block: the corpus rounds each value to `1.0E-4` pixel.
const STEPS_PER_BLOCK: f64 = 16.0 * 10_000.0;

/// Widens a shape coordinate of the definitions, an f32 value in block units, to the f64 value
/// of the corpus: the nearest f64 to the value on the grid of `1.0E-4` pixel. A value that the
/// f32 holds exactly, for example any multiple of 1/32, widens to itself.
#[must_use]
pub fn widen(value: f32) -> f64 {
    (f64::from(value) * STEPS_PER_BLOCK).round() / STEPS_PER_BLOCK
}

/// True when the coordinate is a whole number of pixels.
#[must_use]
pub fn on_pixel_grid(value: f64) -> bool {
    (value * 16.0).fract() == 0.0
}

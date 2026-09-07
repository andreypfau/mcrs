pub mod gradient;
pub mod normal;
pub mod perlin;
pub mod simplex;
pub mod stack;

use crate::interval::Interval;
use crate::volume::Volume;

/// What every noise answers about itself. Vanilla's `Noise` interface.
pub trait Noise {
    fn range(&self) -> Interval;
    fn sample(&self, x: f64, y: f64, z: f64) -> f32;
    /// `unwrapped_ys` is read only by the smeared lattice; the plain one ignores it.
    fn sample_column(&self, x: f64, z: f64, ys: &[f64], unwrapped_ys: &[f64], out: &mut [f32]);
    fn add_to_volume(
        &self,
        out: &mut [f32],
        volume: &Volume,
        xz_scale: f64,
        y_scale: f64,
        amplitude: f32,
    );
    /// Whether [`Self::sample_column`] reads `unwrapped_ys`.
    const NEEDS_UNWRAPPED: bool;
}


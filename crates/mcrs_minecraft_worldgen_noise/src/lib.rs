pub mod blended;
pub mod gradient;
pub mod interval;
pub mod jmath;
pub mod kernel;
pub mod normal;
pub mod perlin;
pub mod proto;
pub mod sample_grid;
pub mod simplex;
pub mod stack;
pub mod strata;

pub use interval::Interval;
pub use sample_grid::{Axis, SampleGrid};

/// What every noise answers about itself. Vanilla's `Noise` interface.
///
/// A coordinate arrives unfolded: each implementation applies [`gradient::wrap`]
/// to its own inputs, as vanilla's do, so the smear can still see the value it
/// quantises against.
pub trait Noise {
    fn range(&self) -> Interval;
    fn get(&self, x: f64, y: f64, z: f64) -> f32;
    /// [`Self::get`] over a run of positions sharing `x` and `z`, which lets the
    /// lattice hashes hoist across the run.
    fn get_column(&self, x: f64, z: f64, ys: &[f64], out: &mut [f32]);
    fn add_to_volume(
        &self,
        out: &mut [f32],
        volume: &SampleGrid,
        xz_scale: f64,
        y_scale: f64,
        amplitude: f32,
    );
}

pub mod combined;
pub mod simplex;
pub mod simplex_octave;

pub use combined::PerlinOctaveNoiseCombined;
pub use simplex::SimplexNoise;
pub use simplex_octave::SimplexOctaveNoise;

use crate::noise::improved_noise::ImprovedNoise;
use crate::noise::octave_perlin_noise::OctavePerlinNoise;

pub type BetaPerlinNoise = ImprovedNoise<f64, true>;
pub type ModernPerlinNoise = ImprovedNoise<f32, false>;
pub type BetaOctavePerlinNoise = OctavePerlinNoise<f64, true>;
pub type ModernOctavePerlinNoise = OctavePerlinNoise<f32, false>;

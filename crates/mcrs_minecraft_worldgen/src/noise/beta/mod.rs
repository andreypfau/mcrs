pub mod combined;
pub mod simplex_octave;

pub use combined::PerlinOctaveNoiseCombined;
pub use crate::noise::simplex::SimplexNoise;
pub use simplex_octave::SimplexOctaveNoise;

use crate::noise::improved_noise::ImprovedNoise;
use crate::noise::octave_perlin_noise::OctavePerlinNoise;

pub type BetaPerlinNoise = ImprovedNoise<f64>;
pub type ModernPerlinNoise = ImprovedNoise<f32>;
pub type BetaOctavePerlinNoise = OctavePerlinNoise<f64>;
pub type ModernOctavePerlinNoise = OctavePerlinNoise<f32>;

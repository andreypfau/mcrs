use crate::noise::improved_noise::ImprovedNoise;
use crate::noise::octave_perlin_noise::OctavePerlinNoise;

pub type BetaPerlinNoise = ImprovedNoise<f64, true>;
pub type ModernPerlinNoise = ImprovedNoise<f32, false>;
pub type BetaOctavePerlinNoise = OctavePerlinNoise<f64, true>;
pub type ModernOctavePerlinNoise = OctavePerlinNoise<f32, false>;

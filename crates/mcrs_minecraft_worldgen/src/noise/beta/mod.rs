use crate::noise::improved_noise::ImprovedNoise;
use crate::noise::octave_perlin_noise::OctavePerlinNoise;

pub type BetaPerlinNoise = ImprovedNoise<f64, true>;
pub type ModernPerlinNoise = ImprovedNoise<f32, false>;

pub mod fixtures {
    pub mod seed_845 {}
}

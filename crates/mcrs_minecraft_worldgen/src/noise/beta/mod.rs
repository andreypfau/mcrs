pub mod octave;
pub mod perlin;
pub mod simplex_octave;

pub use crate::noise::simplex::SimplexNoise;
pub use octave::BetaOctaveNoise;
pub use perlin::BetaPerlinNoise;
pub use simplex_octave::SimplexOctaveNoise;

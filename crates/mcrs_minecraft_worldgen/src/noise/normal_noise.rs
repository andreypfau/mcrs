use crate::noise::beta::simplex_octave::SimplexOctaveNoise;
use crate::noise::octave_perlin_noise::OctavePerlinNoise;
use mcrs_random::Random;

const INPUT_FACTOR: f32 = 1.0181268882175227;

#[derive(Clone, Debug, PartialEq)]
pub struct NormalNoise {
    first: OctavePerlinNoise<f32>,
    second: OctavePerlinNoise<f32>,
    value_factor: f32,
    max_value: f32,
}

/// Beta terrain 2D octave noise (scale/depth). Samples at noise-cell coordinates
/// (block >> 2, matching Java's per-cell sampling) with an id-intrinsic frequency;
/// y is ignored entirely.
#[derive(Clone, Debug, PartialEq)]
pub struct BetaOctave2dNoise {
    noise: OctavePerlinNoise<f32>,
    frequency: f32,
    max_value: f32,
}

/// Beta climate 2D simplex noise (temperature/vegetation/detail). Samples at block
/// coordinates with id-intrinsic scale/lacunarity constants; y is ignored entirely.
#[derive(Clone, Debug, PartialEq)]
pub struct BetaSimplex2dNoise {
    noise: SimplexOctaveNoise,
    scale: f64,
    lacunarity: f64,
    max_value: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NoiseSampler {
    Normal(NormalNoise),
    BetaOctave2d(BetaOctave2dNoise),
    BetaSimplex2d(BetaSimplex2dNoise),
}

impl NoiseSampler {
    pub fn new<R>(random: &mut R, first_octave: i32, amplitudes: Vec<f32>) -> Self
    where
        R: Random,
    {
        let first =
            OctavePerlinNoise::<f32>::new(random, first_octave, amplitudes.clone(), random.is_legacy());
        let second =
            OctavePerlinNoise::<f32>::new(random, first_octave, amplitudes.clone(), random.is_legacy());
        let mut min = f32::INFINITY;
        let mut max = f32::NEG_INFINITY;
        for (i, value) in amplitudes.iter().enumerate() {
            if *value != 0.0 {
                min = min.min(i as f32);
                max = max.max(i as f32);
            }
        }

        let expected_deviation = 0.1 * (1.0 + 1.0 / (max - min + 1.0));
        let value_factor = (1.0 / 6.0) / expected_deviation;
        let max_value = (first.max_value() + second.max_value()) * value_factor;
        Self::Normal(NormalNoise {
            first,
            second,
            value_factor,
            max_value,
        })
    }

    pub fn beta_octave_2d(noise: OctavePerlinNoise<f32>, frequency: f32, max_value: f32) -> Self {
        Self::BetaOctave2d(BetaOctave2dNoise {
            noise,
            frequency,
            max_value,
        })
    }

    pub fn beta_simplex_2d(
        noise: SimplexOctaveNoise,
        scale: f64,
        lacunarity: f64,
        max_value: f32,
    ) -> Self {
        Self::BetaSimplex2d(BetaSimplex2dNoise {
            noise,
            scale,
            lacunarity,
            max_value,
        })
    }

    #[inline]
    pub fn max_value(&self) -> f32 {
        match self {
            Self::Normal(n) => n.max_value,
            Self::BetaOctave2d(n) => n.max_value,
            Self::BetaSimplex2d(n) => n.max_value,
        }
    }

    pub fn get(&self, x: f32, y: f32, z: f32) -> f32 {
        match self {
            Self::Normal(n) => {
                let x2 = x * INPUT_FACTOR;
                let y2 = y * INPUT_FACTOR;
                let z2 = z * INPUT_FACTOR;
                (n.first.get(x, y, z) + n.second.get(x2, y2, z2)) * n.value_factor
            }
            Self::BetaOctave2d(n) => {
                let noise_x = ((x as i32) >> 2) as f32;
                let noise_z = ((z as i32) >> 2) as f32;
                n.noise.sample_xz(noise_x, noise_z, n.frequency, n.frequency)
            }
            Self::BetaSimplex2d(n) => n
                .noise
                .sample(x as f64, z as f64, n.scale, n.scale, n.lacunarity, 0.5)
                as f32,
        }
    }

    /// Batch evaluate NoiseSampler at multiple positions (zero heap allocation).
    /// Evaluates both inner OctavePerlinNoise instances in batch, then combines.
    #[cfg(feature = "batch-noise")]
    pub fn get_batch(&self, positions: &[(f32, f32, f32)], results: &mut [f32]) {
        let n = match self {
            Self::Normal(n) => n,
            _ => {
                for (r, &(x, y, z)) in results.iter_mut().zip(positions) {
                    *r = self.get(x, y, z);
                }
                return;
            }
        };
        const MAX_BATCH: usize = 16;
        let len = positions.len();
        debug_assert_eq!(len, results.len());
        debug_assert!(len <= MAX_BATCH);

        // Build second-set positions (scaled by INPUT_FACTOR) on stack
        let mut second_positions = [(0.0f32, 0.0f32, 0.0f32); MAX_BATCH];
        for i in 0..len {
            let (x, y, z) = positions[i];
            second_positions[i] = (x * INPUT_FACTOR, y * INPUT_FACTOR, z * INPUT_FACTOR);
        }

        // Evaluate first OctavePerlinNoise in batch
        let mut first_results = [0.0f32; MAX_BATCH];
        n.first.get_batch(positions, &mut first_results[..len]);

        // Evaluate second OctavePerlinNoise in batch
        let mut second_results = [0.0f32; MAX_BATCH];
        n.second
            .get_batch(&second_positions[..len], &mut second_results[..len]);

        // Combine: (first + second) * value_factor
        for i in 0..len {
            results[i] = (first_results[i] + second_results[i]) * n.value_factor;
        }
    }
}

// #[cfg(test)]
// mod test {
//     use crate::noise::normal_noise::NoiseSampler;
//     use mcrs_random::legacy::LegacyRandom;
//
//     #[test]
//     fn sample() {
//         let mut random = LegacyRandom::new(82);
//         let noise = NoiseSampler::new(&mut random, -6, vec![1.0, 1.0]);
//         assert_eq!(
//             format!("{:.4}", noise.get(0.0, 0.0, 0.0)),
//             format!("{:.4}", -0.11173738673691287)
//         );
//         assert_eq!(
//             format!("{:.4}", noise.get(0.5, 4.0, -2.0)),
//             format!("{:.4}", -0.12418270136523879)
//         );
//         assert_eq!(
//             format!("{:.4}", noise.get(-204.0, 28.0, 12.0)),
//             format!("{:.4}", -0.593348747968403)
//         );
//     }
//
//     #[cfg(feature = "batch-noise")]
//     #[test]
//     fn get_batch_matches_scalar() {
//         let mut random = LegacyRandom::new(82);
//         let noise = NoiseSampler::new(&mut random, -6, vec![1.0, 1.0]);
//
//         let positions = [
//             (0.0, 0.0, 0.0),
//             (0.5, 4.0, -2.0),
//             (-204.0, 28.0, 12.0),
//             (50.0, 25.0, -50.0),
//             (1000.0, 64.0, 1000.0),
//         ];
//         let mut batch_results = [0.0f32; 5];
//         noise.get_batch(&positions, &mut batch_results);
//
//         for (i, &(x, y, z)) in positions.iter().enumerate() {
//             let scalar = noise.get(x, y, z);
//             assert_eq!(
//                 batch_results[i], scalar,
//                 "Mismatch at position {}: batch={}, scalar={}",
//                 i, batch_results[i], scalar
//             );
//         }
//     }
// }

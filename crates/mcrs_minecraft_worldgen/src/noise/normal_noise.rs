use crate::noise::beta::simplex_octave::SimplexOctaveNoise;
use crate::noise::improved_noise::ImprovedNoise;
use crate::noise::octave_perlin_noise::OctavePerlinNoise;
use mcrs_minecraft_random::Random;

const INPUT_FACTOR: f64 = 1.0181268882175227;
const TARGET_DEVIATION: f64 = 0.3333333333333333;
const PERLIN_STANDARD_DEVIATION: f64 = 0.2702247831245211;

#[derive(Clone, Debug, PartialEq)]
struct Layer {
    noise: ImprovedNoise<f32>,
    frequency: f64,
    amplitude: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NormalNoise {
    layers: Vec<Layer>,
    max_value: f32,
}

fn build_layers(
    first: OctavePerlinNoise<f32>,
    second: OctavePerlinNoise<f32>,
    base_octave: i32,
    amplitude_of: impl Fn(usize) -> f32,
) -> Vec<Layer> {
    let first = first.into_octave_samplers();
    let second = second.into_octave_samplers();
    let mut layers = Vec::with_capacity(first.len() * 2);
    for (i, (a, b)) in first.into_iter().zip(second).enumerate() {
        let (Some(a), Some(b)) = (a, b) else { continue };
        let frequency = 2.0f64.powi(base_octave + i as i32);
        let amplitude = amplitude_of(i);
        layers.push(Layer {
            noise: a,
            frequency,
            amplitude,
        });
        layers.push(Layer {
            noise: b,
            frequency: frequency * INPUT_FACTOR,
            amplitude,
        });
    }
    layers
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
        let first = OctavePerlinNoise::<f32>::new(
            random,
            first_octave,
            amplitudes.clone(),
            random.is_legacy(),
        );
        let second = OctavePerlinNoise::<f32>::new(
            random,
            first_octave,
            amplitudes.clone(),
            random.is_legacy(),
        );
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
        let base_persistence = first.persistence();
        let octave_count = amplitudes.len() as i32;
        let persistence = 2.0f64.powi(octave_count - 1) / (2.0f64.powi(octave_count) - 1.0);
        let input_deviation = deviation(
            amplitudes
                .iter()
                .enumerate()
                .map(|(i, a)| persistence * 0.5f64.powi(i as i32) * *a as f64),
        );
        let layers = build_layers(first, second, first_octave, |i| {
            let persistence = base_persistence * 0.5f32.powi(i as i32);
            persistence * amplitudes[i] * value_factor
        });
        Self::Normal(NormalNoise {
            max_value: symmetric_bound(
                3.0 * std::f64::consts::SQRT_2 * input_deviation * value_factor as f64,
            ),
            layers,
        })
    }

    pub fn from_params<R>(
        random: &mut R,
        base_octave: i32,
        octave_amplitudes: Vec<f64>,
        base_amplitude: f64,
    ) -> Self
    where
        R: Random,
    {
        let modifiers: Vec<f32> = octave_amplitudes.iter().map(|a| *a as f32).collect();
        let first = OctavePerlinNoise::<f32>::new(
            random,
            base_octave,
            modifiers.clone(),
            random.is_legacy(),
        );
        let second =
            OctavePerlinNoise::<f32>::new(random, base_octave, modifiers, random.is_legacy());

        let count = octave_amplitudes.len() as i32;
        let mut amplitude = base_amplitude * (2.0f64.powi(count - 1) / (2.0f64.powi(count) - 1.0));
        let mut octave_amplitude = Vec::with_capacity(octave_amplitudes.len());
        for modifier in &octave_amplitudes {
            octave_amplitude.push(if *modifier != 0.0 {
                Some(amplitude * *modifier)
            } else {
                None
            });
            amplitude *= 0.5;
        }

        let target_amplitude = compensated_sum(octave_amplitude.iter().flatten().map(|a| a.abs()));
        let input_deviation = deviation(octave_amplitude.iter().flatten().copied());
        let normalization_factor = if input_deviation == 0.0 {
            0.0
        } else {
            (target_amplitude * TARGET_DEVIATION) / (input_deviation * std::f64::consts::SQRT_2)
        };

        let layers = build_layers(first, second, base_octave, |i| {
            (normalization_factor * octave_amplitude[i].unwrap_or(0.0)) as f32
        });
        Self::Normal(NormalNoise {
            max_value: symmetric_bound(target_amplitude),
            layers,
        })
    }

    pub fn octave_count(&self) -> usize {
        match self {
            NoiseSampler::Normal(n) => n.layers.len(),
            NoiseSampler::BetaOctave2d(n) => n.noise.octave_count(),
            NoiseSampler::BetaSimplex2d(_) => 1,
        }
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

    pub fn get(&self, x: f64, y: f64, z: f64) -> f32 {
        match self {
            Self::Normal(n) => {
                let mut value = 0.0f32;
                for layer in &n.layers {
                    let f = layer.frequency;
                    value += layer.amplitude
                        * layer
                            .noise
                            .sample(wrap(x * f), wrap(y * f), wrap(z * f), 0.0, 0.0);
                }
                value
            }
            Self::BetaOctave2d(n) => {
                let noise_x = ((x as i32) >> 2) as f32;
                let noise_z = ((z as i32) >> 2) as f32;
                n.noise
                    .sample_xz(noise_x, noise_z, n.frequency, n.frequency)
            }
            Self::BetaSimplex2d(n) => {
                n.noise.sample(x, z, n.scale, n.scale, n.lacunarity, 0.5) as f32
            }
        }
    }

    /// Batch evaluate NoiseSampler at multiple positions (zero heap allocation).
    /// Evaluates both inner OctavePerlinNoise instances in batch, then combines.
    #[cfg(feature = "batch-noise")]
    pub fn get_batch(&self, positions: &[(f64, f64, f64)], results: &mut [f32]) {
        let n = match self {
            Self::Normal(n) => n,
            _ => {
                for (r, &(x, y, z)) in results.iter_mut().zip(positions) {
                    *r = self.get(x, y, z);
                }
                return;
            }
        };
        use crate::density_function::MAX_BATCH;
        let len = positions.len();
        debug_assert_eq!(len, results.len());
        debug_assert!(len <= MAX_BATCH);
        results[..len].iter_mut().for_each(|r| *r = 0.0);

        let mut scaled = [(0.0f64, 0.0f64, 0.0f64); MAX_BATCH];
        let mut layer_results = [0.0f32; MAX_BATCH];
        for layer in &n.layers {
            let f = layer.frequency;
            for i in 0..len {
                let (x, y, z) = positions[i];
                scaled[i] = (wrap(x * f), wrap(y * f), wrap(z * f));
            }
            layer
                .noise
                .sample_batch(&scaled[..len], 0.0, &[], &mut layer_results[..len]);
            for i in 0..len {
                results[i] += layer.amplitude * layer_results[i];
            }
        }
    }
}

#[inline(always)]
fn wrap(value: f64) -> f64 {
    OctavePerlinNoise::<f32>::maintain_precission(value)
}

fn deviation(amplitudes: impl Iterator<Item = f64>) -> f64 {
    let mut variance = 0.0f64;
    for a in amplitudes {
        let layer_deviation = PERLIN_STANDARD_DEVIATION * a.abs();
        variance += layer_deviation * layer_deviation;
    }
    variance.sqrt()
}

// Deliberately a six-sigma statistical bound on the summed octaves, not the analytically
// rigorous extreme (which is ~2x wider). Branch elimination consumes it, and widening it
// to the rigorous form silently changes generated terrain.
fn symmetric_bound(target_amplitude: f64) -> f32 {
    (target_amplitude * TARGET_DEVIATION * 6.0) as f32
}

// The reference totals the octave amplitudes with compensated summation, and that total
// feeds every layer's folded float factor — a plain sum can land an ulp off and shift one.
fn compensated_sum(values: impl Iterator<Item = f64>) -> f64 {
    let mut sum = 0.0f64;
    let mut compensation = 0.0f64;
    let mut simple = 0.0f64;
    for value in values {
        let corrected = value - compensation;
        let next = sum + corrected;
        compensation = (next - sum) - corrected;
        sum = next;
        simple += value;
    }
    let total = sum - compensation;
    if total.is_nan() && simple.is_infinite() {
        simple
    } else {
        total
    }
}

// #[cfg(test)]
// mod test {
//     use crate::noise::normal_noise::NoiseSampler;
//     use mcrs_minecraft_random::legacy::LegacyRandom;
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

#[cfg(test)]
mod bound_tests {
    use super::NoiseSampler;
    use mcrs_minecraft_random::legacy::LegacyRandom;

    #[test]
    fn reported_range_is_the_six_sigma_bound() {
        let continentalness = NoiseSampler::from_params(
            &mut LegacyRandom::new(1),
            -9,
            vec![1.0, 1.0, 2.0, 2.0, 2.0, 1.0, 1.0, 1.0, 1.0],
            0.8880832896205223,
        );
        assert_eq!(continentalness.max_value(), 2.1654634);

        let legacy_temperature = NoiseSampler::new(&mut LegacyRandom::new(82), -7, vec![1.0, 1.0]);
        assert_eq!(legacy_temperature.max_value(), 1.898946);

        let legacy_offset = NoiseSampler::new(&mut LegacyRandom::new(82), 0, vec![0.0]);
        assert_eq!(legacy_offset.max_value(), 0.0);
    }
}

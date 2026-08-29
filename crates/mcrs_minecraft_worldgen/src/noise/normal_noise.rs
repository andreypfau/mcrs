use crate::noise::beta::simplex_octave::SimplexOctaveNoise;
use crate::noise::improved_noise::ImprovedNoise;
use crate::noise::octave_perlin_noise::OctavePerlinNoise;
use crate::proto::Normalization;
use crate::volume::Volume;
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

        let value_factor = parity_normalization_factor(1.0, (max - min) as f64) as f32;
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
        normalize: Normalization,
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
        let mut amplitude = match normalize {
            Normalization::Disabled => base_amplitude,
            _ => base_amplitude * (2.0f64.powi(count - 1) / (2.0f64.powi(count) - 1.0)),
        };
        let mut octave_amplitude = Vec::with_capacity(octave_amplitudes.len());
        for modifier in &octave_amplitudes {
            octave_amplitude.push(if *modifier != 0.0 {
                Some(amplitude * *modifier)
            } else {
                None
            });
            amplitude *= 0.5;
        }

        let mut target_amplitude =
            compensated_sum(octave_amplitude.iter().flatten().map(|a| a.abs()));
        let input_deviation = deviation(octave_amplitude.iter().flatten().copied());
        let mut normalization_factor = if input_deviation == 0.0 {
            0.0
        } else {
            (target_amplitude * TARGET_DEVIATION) / (input_deviation * std::f64::consts::SQRT_2)
        };
        if normalize == Normalization::Legacy && normalization_factor != 0.0 {
            let lowest = octave_amplitudes.iter().position(|a| *a != 0.0).unwrap();
            let highest = octave_amplitudes.iter().rposition(|a| *a != 0.0).unwrap();
            let parity = parity_normalization_factor(base_amplitude, (highest - lowest) as f64);
            target_amplitude *= parity / normalization_factor;
            normalization_factor = parity;
        }

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
}

#[derive(Default)]
pub struct ColumnScratch {
    scaled: Vec<f64>,
    layer: Vec<f32>,
}

impl NoiseSampler {
    /// [`NoiseSampler::get`] over a run of positions sharing `x` and `z`, which
    /// lets each octave hoist its lattice hashes across the run.
    pub fn get_column(
        &self,
        x: f64,
        z: f64,
        ys: &[f64],
        out: &mut [f32],
        scratch: &mut ColumnScratch,
    ) {
        let Self::Normal(n) = self else {
            for (slot, &y) in out.iter_mut().zip(ys) {
                *slot = self.get(x, y, z);
            }
            return;
        };
        out.fill(0.0);
        scratch.scaled.clear();
        scratch.scaled.resize(ys.len(), 0.0);
        scratch.layer.clear();
        scratch.layer.resize(ys.len(), 0.0);
        for layer in &n.layers {
            let f = layer.frequency;
            for (slot, &y) in scratch.scaled.iter_mut().zip(ys) {
                *slot = wrap(y * f);
            }
            layer.noise.sample_column(
                wrap(x * f),
                wrap(z * f),
                &scratch.scaled,
                0.0,
                &[],
                &mut scratch.layer,
            );
            for (slot, &sampled) in out.iter_mut().zip(scratch.layer.iter()) {
                *slot += layer.amplitude * sampled;
            }
        }
    }
}

impl NoiseSampler {
    /// Accumulates `amplitude * get(...)` over `volume`.
    ///
    /// Each layer receives `xz_scale * frequency` *before* the block multiply, where
    /// [`NoiseSampler::get`] multiplies the block first and the frequency second. At
    /// the `1.0181268882175227` frequency ratio of every second sub-noise the two
    /// products differ by an f64 ulp; vanilla's scalar and volume paths disagree the
    /// same way, so this must not be unified with `get`.
    pub fn add_to_volume(
        &self,
        out: &mut [f32],
        volume: &Volume,
        xz_scale: f64,
        y_scale: f64,
        amplitude: f32,
    ) {
        let Self::Normal(n) = self else {
            let size = volume.size();
            let mut i = 0usize;
            for iz in 0..size.z {
                let z = volume.block_z(iz) as f64 * xz_scale;
                for ix in 0..size.x {
                    let x = volume.block_x(ix) as f64 * xz_scale;
                    for iy in 0..size.y {
                        out[i] += amplitude * self.get(x, volume.block_y(iy) as f64 * y_scale, z);
                        i += 1;
                    }
                }
            }
            return;
        };
        for layer in &n.layers {
            layer.noise.add_to_volume(
                out,
                volume,
                xz_scale * layer.frequency,
                y_scale * layer.frequency,
                0.0,
                amplitude * layer.amplitude,
            );
        }
    }
}

#[inline(always)]
fn wrap(value: f64) -> f64 {
    OctavePerlinNoise::<f32>::maintain_precission(value)
}

fn parity_normalization_factor(base_amplitude: f64, octave_span: f64) -> f64 {
    let expected_deviation = 0.1 * (1.0 + 1.0 / (octave_span + 1.0));
    base_amplitude * 0.5 * TARGET_DEVIATION / expected_deviation
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

#[cfg(test)]
mod bound_tests {
    use super::{NoiseSampler, Normalization};
    use mcrs_minecraft_random::legacy::LegacyRandom;

    const CONTINENTALNESS_MODIFIERS: [f64; 9] = [1.0, 1.0, 2.0, 2.0, 2.0, 1.0, 1.0, 1.0, 1.0];
    const CONTINENTALNESS_AMPLITUDE: f64 = 0.8880832896205223;
    const GAPPED_MODIFIERS: [f64; 5] = [1.0, 0.0, 0.0, 0.5, 1.0];

    fn sampler(modifiers: &[f64], base_amplitude: f64, normalize: Normalization) -> NoiseSampler {
        NoiseSampler::from_params(
            &mut LegacyRandom::new(1),
            -9,
            modifiers.to_vec(),
            base_amplitude,
            normalize,
        )
    }

    fn octave_factors(sampler: &NoiseSampler) -> Vec<f32> {
        let NoiseSampler::Normal(noise) = sampler else {
            unreachable!()
        };
        noise
            .layers
            .chunks(2)
            .map(|pair| pair[0].amplitude)
            .collect()
    }

    #[test]
    fn disabled_normalization_keeps_the_base_amplitude_unscaled() {
        let noise = sampler(
            &CONTINENTALNESS_MODIFIERS,
            CONTINENTALNESS_AMPLITUDE,
            Normalization::Disabled,
        );
        assert_eq!(
            octave_factors(&noise),
            vec![
                1.5,
                0.75,
                0.75,
                0.375,
                0.1875,
                0.046875,
                0.0234375,
                0.01171875,
                0.005859375
            ]
        );
        assert_eq!(noise.max_value(), 4.322468);

        let gapped = sampler(&GAPPED_MODIFIERS, 1.0, Normalization::Disabled);
        assert_eq!(
            octave_factors(&gapped),
            vec![0.97746503, 0.061091565, 0.061091565]
        );
        assert_eq!(gapped.max_value(), 2.25);
    }

    #[test]
    fn enabled_normalization_prescales_the_octave_amplitudes() {
        let noise = sampler(
            &CONTINENTALNESS_MODIFIERS,
            CONTINENTALNESS_AMPLITUDE,
            Normalization::Enabled,
        );
        assert_eq!(
            octave_factors(&noise),
            vec![
                0.7514677,
                0.37573385,
                0.37573385,
                0.18786693,
                0.09393346,
                0.023483366,
                0.011741683,
                0.0058708414,
                0.0029354207
            ]
        );
        assert_eq!(noise.max_value(), 2.1654634);

        let gapped = sampler(&GAPPED_MODIFIERS, 1.0, Normalization::Enabled);
        assert_eq!(
            octave_factors(&gapped),
            vec![0.50449806, 0.03153113, 0.03153113]
        );
        assert_eq!(gapped.max_value(), 1.1612903);
    }

    #[test]
    fn legacy_normalization_swaps_in_the_parity_factor() {
        let noise = sampler(
            &CONTINENTALNESS_MODIFIERS,
            CONTINENTALNESS_AMPLITUDE,
            Normalization::Legacy,
        );
        assert_eq!(
            octave_factors(&noise),
            vec![
                0.5926765,
                0.29633826,
                0.29633826,
                0.14816913,
                0.074084565,
                0.018521141,
                0.009260571,
                0.0046302853,
                0.0023151427
            ]
        );
        assert_eq!(noise.max_value(), 1.7078835);

        let gapped = sampler(&GAPPED_MODIFIERS, 1.0, Normalization::Legacy);
        assert_eq!(
            octave_factors(&gapped),
            vec![0.71684587, 0.044802867, 0.044802867]
        );
        assert_eq!(gapped.max_value(), 1.650088);
    }

    #[test]
    fn a_silent_noise_normalizes_to_nothing_in_every_mode() {
        for normalize in [
            Normalization::Disabled,
            Normalization::Enabled,
            Normalization::Legacy,
        ] {
            let noise = sampler(&[0.0], 1.0, normalize);
            assert_eq!(octave_factors(&noise), Vec::<f32>::new());
            assert_eq!(noise.max_value(), 0.0);
            assert_eq!(noise.get(12.0, -30.0, 7.0), 0.0);
        }
    }

    // Unlike the octave factors and ranges above, these bits are pinned from our own
    // sampler, not derived from the reference: a drift guard, not a parity check.
    #[test]
    fn the_modes_sample_apart() {
        let positions = [(0.0, 0.0, 0.0), (0.5, 4.0, -2.0), (-204.0, 28.0, 12.0)];
        let expected: [[u32; 3]; 3] = [
            [0x3e68047a, 0x3e2b16cf, 0xbf105330],
            [0x3de878b7, 0x3dab6c85, 0xbe909b7e],
            [0x3db75933, 0x3d87335d, 0xbe6419f3],
        ];
        for (mode, bits) in [
            Normalization::Disabled,
            Normalization::Enabled,
            Normalization::Legacy,
        ]
        .into_iter()
        .zip(expected)
        {
            let noise = sampler(&CONTINENTALNESS_MODIFIERS, CONTINENTALNESS_AMPLITUDE, mode);
            let actual: Vec<u32> = positions
                .iter()
                .map(|(x, y, z)| noise.get(*x, *y, *z).to_bits())
                .collect();
            assert_eq!(actual, bits.to_vec(), "{mode:?}");
        }
    }
}

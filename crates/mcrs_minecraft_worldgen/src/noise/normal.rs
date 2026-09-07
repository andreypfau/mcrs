use crate::interval::Interval;
use crate::noise::beta::octave::BetaOctaveNoise;
use crate::noise::beta::simplex_octave::SimplexOctaveNoise;
use crate::noise::gradient::GradientNoise;
use crate::noise::perlin::PerlinNoise;
use crate::noise::stack::{ColumnScratch, NoiseStack};
use crate::proto::NoiseParam;
use crate::proto::noise::{deviation, parity_normalization_factor};
use crate::volume::Volume;
use mcrs_minecraft_random::Random;

/// Every second sub-noise is offset by this ratio so the pair decorrelates.
const INPUT_FACTOR: f64 = 1.0181268882175227;
const TARGET_DEVIATION: f64 = 0.3333333333333333;

/// Beta terrain 2D octave noise (scale/depth). Samples at noise-cell coordinates
/// (block >> 2, matching Java's per-cell sampling) with an id-intrinsic frequency;
/// y is ignored entirely.
#[derive(Clone, Debug, PartialEq)]
pub struct BetaOctave2dNoise {
    noise: BetaOctaveNoise,
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
    Normal(NoiseStack<PerlinNoise>),
    BetaOctave2d(BetaOctave2dNoise),
    BetaSimplex2d(BetaSimplex2dNoise),
}

/// The two decorrelated halves of one noise, drawn back to back. The trailing
/// `fork` inside each draw is what separates the second from the first.
fn draw_pair<R: Random>(
    random: &mut R,
    base_octave: i32,
    modifiers: &[f64],
) -> (Vec<Option<GradientNoise>>, Vec<Option<GradientNoise>>) {
    let legacy = random.is_legacy();
    let first = GradientNoise::octaves(random, base_octave, modifiers, legacy);
    let second = GradientNoise::octaves(random, base_octave, modifiers, legacy);
    (first, second)
}

fn build_stack(
    first: Vec<Option<GradientNoise>>,
    second: Vec<Option<GradientNoise>>,
    base_octave: i32,
    range: Interval,
    amplitude_of: impl Fn(usize) -> f32,
) -> NoiseStack<PerlinNoise> {
    let mut stack = NoiseStack::builder();
    for (i, (a, b)) in first.into_iter().zip(second).enumerate() {
        let (Some(a), Some(b)) = (a, b) else { continue };
        let frequency = 2.0f64.powi(base_octave + i as i32);
        let amplitude = amplitude_of(i);
        stack.add(PerlinNoise::from_gradient(a), frequency, amplitude);
        stack.add(
            PerlinNoise::from_gradient(b),
            frequency * INPUT_FACTOR,
            amplitude,
        );
    }
    stack.build_with_range(range)
}

impl NoiseSampler {
    /// Vanilla's `NormalNoise.createParity`: the pre-parameters shape, where the
    /// amplitudes are the octave modifiers and the base amplitude is implied.
    pub fn new<R: Random>(random: &mut R, first_octave: i32, amplitudes: Vec<f32>) -> Self {
        let modifiers: Vec<f64> = amplitudes.iter().map(|a| *a as f64).collect();
        let (first, second) = draw_pair(random, first_octave, &modifiers);

        let mut min = f32::INFINITY;
        let mut max = f32::NEG_INFINITY;
        for (i, value) in amplitudes.iter().enumerate() {
            if *value != 0.0 {
                min = min.min(i as f32);
                max = max.max(i as f32);
            }
        }
        let value_factor = parity_normalization_factor(1.0, (max - min) as f64) as f32;

        // The persistence is taken in the value width, not widened from f64: the
        // quotient of two exact powers of two still rounds differently either way.
        let len = amplitudes.len() as f32;
        let base_persistence = 2.0f32.powf(len - 1.0) / (2.0f32.powf(len) - 1.0);

        let octave_count = amplitudes.len() as i32;
        let persistence = 2.0f64.powi(octave_count - 1) / (2.0f64.powi(octave_count) - 1.0);
        let input_deviation = deviation(
            amplitudes
                .iter()
                .enumerate()
                .map(|(i, a)| persistence * 0.5f64.powi(i as i32) * *a as f64),
        );
        let range = symmetric_bound(
            3.0 * std::f64::consts::SQRT_2 * input_deviation * value_factor as f64,
        );

        Self::Normal(build_stack(first, second, first_octave, range, |i| {
            base_persistence * 0.5f32.powi(i as i32) * amplitudes[i] * value_factor
        }))
    }

    /// Vanilla's `NormalNoise.create`: the parameters have already decided every
    /// octave's weight and the declared range, so this only draws the lattices.
    pub fn from_params<R: Random>(random: &mut R, params: &NoiseParam) -> Self {
        let modifiers = params.octave_amplitudes();
        let (first, second) = draw_pair(random, params.base_octave, &modifiers);
        let octaves = params.octaves();
        Self::Normal(build_stack(
            first,
            second,
            params.base_octave,
            params.range(),
            |i| (octaves.factor * octaves.amplitudes[i].unwrap_or(0.0)) as f32,
        ))
    }

    pub fn beta_octave_2d(noise: BetaOctaveNoise, frequency: f32, max_value: f32) -> Self {
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

    /// The declared value bound. Vanilla's `Noise.range()`.
    #[inline]
    pub fn range(&self) -> Interval {
        match self {
            Self::Normal(n) => n.range(),
            Self::BetaOctave2d(n) => Interval::symmetric(n.max_value),
            Self::BetaSimplex2d(n) => Interval::symmetric(n.max_value),
        }
    }

    pub fn get(&self, x: f64, y: f64, z: f64) -> f32 {
        match self {
            Self::Normal(n) => n.get(x, y, z),
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
        match self {
            Self::Normal(n) => n.fill_column(out, x, z, ys, scratch),
            _ => {
                for (slot, &y) in out.iter_mut().zip(ys) {
                    *slot = self.get(x, y, z);
                }
            }
        }
    }

    /// Accumulates `amplitude * get(...)` over `volume`.
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
        n.add_to_volume(out, volume, xz_scale, y_scale, amplitude);
    }
}

// Deliberately a six-sigma statistical bound on the summed octaves, not the analytically
// rigorous extreme (which is ~2x wider). Branch elimination consumes it, and widening it
// to the rigorous form silently changes generated terrain.
fn symmetric_bound(target_amplitude: f64) -> Interval {
    Interval::symmetric((target_amplitude * TARGET_DEVIATION * 6.0) as f32)
}

#[cfg(test)]
mod bound_tests {
    use super::NoiseSampler;
    use crate::proto::noise::{NoiseParam, Normalization};
    use crate::proto::HashableF64;
    use mcrs_minecraft_random::legacy::LegacyRandom;

    const CONTINENTALNESS_MODIFIERS: [f64; 9] = [1.0, 1.0, 2.0, 2.0, 2.0, 1.0, 1.0, 1.0, 1.0];
    const CONTINENTALNESS_AMPLITUDE: f64 = 0.8880832896205223;
    const GAPPED_MODIFIERS: [f64; 5] = [1.0, 0.0, 0.0, 0.5, 1.0];

    fn sampler(modifiers: &[f64], base_amplitude: f64, normalize: Normalization) -> NoiseSampler {
        let params = NoiseParam {
            base_octave: -9,
            base_amplitude: HashableF64(base_amplitude),
            octave_count: modifiers.len(),
            normalize,
            amplitude_modifiers: modifiers.iter().map(|m| HashableF64(*m)).collect(),
        };
        NoiseSampler::from_params(&mut LegacyRandom::new(1), &params)
    }

    fn octave_factors(sampler: &NoiseSampler) -> Vec<f32> {
        let NoiseSampler::Normal(noise) = sampler else {
            unreachable!()
        };
        noise.amplitudes().chunks(2).map(|pair| pair[0]).collect()
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
        assert_eq!(noise.range().max(), 4.322468);

        let gapped = sampler(&GAPPED_MODIFIERS, 1.0, Normalization::Disabled);
        assert_eq!(
            octave_factors(&gapped),
            vec![0.97746503, 0.061091565, 0.061091565]
        );
        assert_eq!(gapped.range().max(), 2.25);
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
        assert_eq!(noise.range().max(), 2.1654634);

        let gapped = sampler(&GAPPED_MODIFIERS, 1.0, Normalization::Enabled);
        assert_eq!(
            octave_factors(&gapped),
            vec![0.50449806, 0.03153113, 0.03153113]
        );
        assert_eq!(gapped.range().max(), 1.1612903);
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
        assert_eq!(noise.range().max(), 1.7078835);

        let gapped = sampler(&GAPPED_MODIFIERS, 1.0, Normalization::Legacy);
        assert_eq!(
            octave_factors(&gapped),
            vec![0.71684587, 0.044802867, 0.044802867]
        );
        assert_eq!(gapped.range().max(), 1.650088);
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
            assert_eq!(noise.range().max(), 0.0);
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

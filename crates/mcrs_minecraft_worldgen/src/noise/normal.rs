use crate::interval::Interval;
use crate::noise::gradient::GradientNoise;
use crate::noise::perlin::PerlinNoise;
use crate::noise::stack::{NoiseStack, Octave};
use crate::proto::HashableF64;
use crate::proto::noise::{
    NoiseParam, Normalization, Octaves, declared_range, parity_normalization_factor,
};
use mcrs_minecraft_random::Random;

/// Every second sub-noise is offset by this ratio so the pair decorrelates.
const INPUT_FACTOR: f64 = 1.0181268882175227;

/// The parameters of a `worldgen/noise` entry together with everything they
/// decide before a seed is drawn. Vanilla's `NormalNoise`: a description, not a
/// sampler — [`NormalNoise::create`] draws the lattices and returns the stack.
#[derive(Clone, Debug, PartialEq)]
pub struct NormalNoise {
    params: NoiseParam,
    octaves: Octaves,
}

impl NormalNoise {
    pub fn new(params: NoiseParam) -> Self {
        let octaves = params.octaves();
        Self { params, octaves }
    }

    /// Vanilla's `NormalNoise.createParity`: the pre-parameters shape, where the
    /// amplitudes are the octave modifiers and the base amplitude is whatever
    /// makes the modern normalization reproduce the old one.
    pub fn create_parity(base_octave: i32, amplitudes: &[f64]) -> Self {
        let params = |base_amplitude: f64| NoiseParam {
            base_octave,
            base_amplitude: HashableF64(base_amplitude),
            octave_count: amplitudes.len(),
            normalize: Normalization::Enabled,
            amplitude_modifiers: amplitudes.iter().map(|a| HashableF64(*a)).collect(),
        };
        let probe = params(1.0).octaves();
        let base_amplitude = if probe.factor == 0.0 {
            1.0
        } else {
            let lowest = amplitudes.iter().position(|a| *a != 0.0).unwrap();
            let highest = amplitudes.iter().rposition(|a| *a != 0.0).unwrap();
            parity_normalization_factor(1.0, (highest - lowest) as f64) / probe.factor
        };
        Self::new(params(base_amplitude))
    }

    /// The declared value bound. Vanilla's `Noise.range()`, six sigma on the
    /// summed octaves.
    pub fn range(&self) -> Interval {
        declared_range(self.octaves.target_amplitude)
    }

    /// Draws the two decorrelated halves back to back and lays them out as one
    /// stack, the trailing `fork` inside each draw being what separates them.
    pub fn create<R: Random>(&self, random: &mut R) -> NoiseStack<Octave> {
        let modifiers = self.params.octave_amplitudes();
        let base_octave = self.params.base_octave;
        let draw = |random: &mut R| {
            if random.is_legacy() {
                GradientNoise::legacy_octaves(random, base_octave, &modifiers)
            } else {
                GradientNoise::octaves(random, base_octave, &modifiers)
            }
        };
        let first = draw(random);
        let second = draw(random);

        let mut stack = NoiseStack::builder();
        for (i, (a, b)) in first.into_iter().zip(second).enumerate() {
            let (Some(a), Some(b)) = (a, b) else { continue };
            let frequency = 2.0f64.powi(base_octave + i as i32);
            let amplitude = (self.octaves.factor * self.octaves.amplitudes[i].unwrap_or(0.0)) as f32;
            stack.add(
                Octave::Perlin(PerlinNoise::from_gradient(a)),
                frequency,
                amplitude,
            );
            stack.add(
                Octave::Perlin(PerlinNoise::from_gradient(b)),
                frequency * INPUT_FACTOR,
                amplitude,
            );
        }
        stack.build_with_range(self.range())
    }
}

#[cfg(test)]
mod bound_tests {
    use super::NormalNoise;
    use crate::noise::stack::{NoiseStack, Octave};
    use crate::proto::HashableF64;
    use crate::proto::noise::{NoiseParam, Normalization};
    use mcrs_minecraft_random::legacy::LegacyRandom;

    const CONTINENTALNESS_MODIFIERS: [f64; 9] = [1.0, 1.0, 2.0, 2.0, 2.0, 1.0, 1.0, 1.0, 1.0];
    const CONTINENTALNESS_AMPLITUDE: f64 = 0.8880832896205223;
    const GAPPED_MODIFIERS: [f64; 5] = [1.0, 0.0, 0.0, 0.5, 1.0];

    fn sampler(
        modifiers: &[f64],
        base_amplitude: f64,
        normalize: Normalization,
    ) -> NoiseStack<Octave> {
        let params = NoiseParam {
            base_octave: -9,
            base_amplitude: HashableF64(base_amplitude),
            octave_count: modifiers.len(),
            normalize,
            amplitude_modifiers: modifiers.iter().map(|m| HashableF64(*m)).collect(),
        };
        NormalNoise::new(params).create(&mut LegacyRandom::new(1))
    }

    fn octave_factors(noise: &NoiseStack<Octave>) -> Vec<f32> {
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
    //
    // The fast profile collapses a lattice cell to a line and lands a few ulps
    // away, so it carries its own table. A tolerance wide enough to cover both
    // would stop catching drift in either.
    #[test]
    fn the_modes_sample_apart() {
        let positions = [(0.0, 0.0, 0.0), (0.5, 4.0, -2.0), (-204.0, 28.0, 12.0)];
        #[cfg(not(feature = "fast"))]
        let expected: [[u32; 3]; 3] = [
            [0x3e68047a, 0x3e2b16cf, 0xbf105330],
            [0x3de878b7, 0x3dab6c85, 0xbe909b7e],
            [0x3db75933, 0x3d87335d, 0xbe6419f3],
        ];
        #[cfg(feature = "fast")]
        let expected: [[u32; 3]; 3] = [
            [0x3e680480, 0x3e2b16cf, 0xbf10532f],
            [0x3de878bf, 0x3dab6c84, 0xbe909b7c],
            [0x3db7593b, 0x3d87335e, 0xbe6419f0],
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

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

/// Vanilla's `NormalNoise.createParity`: the pre-parameters shape, where the
/// amplitudes are the octave modifiers and the base amplitude is whatever makes
/// the modern normalization reproduce the old one.
pub fn parity_params(base_octave: i32, amplitudes: &[f64]) -> NoiseParam {
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
    params(base_amplitude)
}

/// [`create`] over the parity shape, which is how every id with no
/// `worldgen/noise` entry of its own is drawn.
pub fn create_parity<R: Random>(
    base_octave: i32,
    amplitudes: &[f64],
    random: &mut R,
) -> NoiseStack<Octave> {
    create(&parity_params(base_octave, amplitudes), random)
}

/// The declared value bound. Vanilla's `Noise.range()`, six sigma on the summed
/// octaves.
pub fn range(params: &NoiseParam) -> Interval {
    declared_range(params.octaves().target_amplitude)
}

/// Vanilla's `NormalNoise`: draws the two decorrelated halves back to back and
/// lays them out as one stack, the trailing `fork` inside each draw being what
/// separates them.
pub fn create<R: Random>(params: &NoiseParam, random: &mut R) -> NoiseStack<Octave> {
    let octaves: Octaves = params.octaves();
    let modifiers = params.octave_amplitudes();
    let base_octave = params.base_octave;
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
        let amplitude = (octaves.factor * octaves.amplitudes[i].unwrap_or(0.0)) as f32;
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
    stack.build_with_range(declared_range(octaves.target_amplitude))
}

#[cfg(test)]
mod bound_tests {
    use super::create;
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
        create(&params, &mut LegacyRandom::new(1))
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

    /// The table this profile is pinned to. The two profiles each carry their
    /// own, because a tolerance wide enough to cover both would stop catching
    /// drift in either.
    fn pinned_samples() -> [[u32; 3]; 3] {
        #[cfg(not(feature = "fast"))]
        return [
            [0x3e68047a, 0x3e2b16cf, 0xbf105330],
            [0x3de878b7, 0x3dab6c85, 0xbe909b7e],
            [0x3db75933, 0x3d87335d, 0xbe6419f3],
        ];
        #[cfg(feature = "fast")]
        return [
            [0x3e68046b, 0x3e2b16eb, 0xbf10532f],
            [0x3de878aa, 0x3dab6ca1, 0xbe909b7e],
            [0x3db75929, 0x3d873374, 0xbe6419f4],
        ];
    }

    /// Unlike the octave factors and ranges above, these bits are pinned from our
    /// own sampler, not derived from the reference: a drift guard, not a parity
    /// check.
    #[test]
    fn the_modes_sample_apart() {
        let positions = [(0.0, 0.0, 0.0), (0.5, 4.0, -2.0), (-204.0, 28.0, 12.0)];
        let expected = pinned_samples();
        for (mode, bits) in [
            Normalization::Disabled,
            Normalization::Enabled,
            Normalization::Legacy,
        ]
        .into_iter()
        .zip(expected)
        {
            let noise = sampler(&CONTINENTALNESS_MODIFIERS, CONTINENTALNESS_AMPLITUDE, mode);
            let actual: Vec<f32> = positions
                .iter()
                .map(|(x, y, z)| noise.get(*x, *y, *z))
                .collect();
            let got: Vec<u32> = actual.iter().map(|v| v.to_bits()).collect();
            assert_eq!(got, bits.to_vec(), "{mode:?}");
        }
    }
}

use crate::noise::gradient::GradientNoise;
use mcrs_minecraft_random::Random;

/// Beta's fbm: octaves walked from the last-constructed to the first, frequency
/// halving each step and each contribution divided by that frequency. Nothing is
/// normalised, unlike [`crate::noise::stack::NoiseStack`].
///
/// The lattices are stored width-free; the sampling method picks the width,
/// because Beta's terrain path computes in double while its 2D scale and depth
/// nodes compute in float.
#[derive(Clone, Debug, PartialEq)]
pub struct BetaOctaveNoise {
    octaves: Vec<Option<GradientNoise>>,
    max_value: f64,
}

impl BetaOctaveNoise {
    pub fn new<T>(random: &mut T, first_octave: i32, octave_count: usize) -> Self
    where
        T: Random + Clone,
    {
        let amplitudes = vec![1.0f64; octave_count];
        let octaves = GradientNoise::octaves(random, first_octave, &amplitudes, true);
        let persistence = 2.0f64.powi(octave_count as i32 - 1) / (2.0f64.powi(octave_count as i32) - 1.0);
        let mut max_value = 0.0;
        let mut factor = persistence;
        for octave in &octaves {
            if octave.is_some() {
                max_value += 2.0 * factor;
            }
            factor *= 0.5;
        }
        Self { octaves, max_value }
    }

    pub fn max_value(&self) -> f64 {
        self.max_value
    }

    #[cfg(test)]
    pub fn octave_count(&self) -> usize {
        self.octaves.iter().filter(|o| o.is_some()).count()
    }

    /// 2D XZ-plane sample for Beta scale/depth nodes, mirroring `NoiseGeneratorOctaves.a(d0,d1)`.
    pub fn sample_xz(&self, x: f32, z: f32, scale_x: f32, scale_z: f32) -> f32 {
        let mut freq = 1.0_f32;
        let mut acc = 0.0_f32;
        for octave in self.octaves.iter().rev() {
            if let Some(lattice) = octave {
                acc += lattice.sample_2d_f32(x * scale_x * freq, z * scale_z * freq) / freq;
            }
            freq /= 2.0;
        }
        acc
    }

    /// 3D sample with separate per-axis scales, matching
    /// `ChunkProviderGenerate`'s `n.a(arr, x, y, z, …, xScale, yScale, zScale)`.
    pub fn sample_xyz_beta(
        &self,
        x: f64,
        y: f64,
        z: f64,
        scale_x: f64,
        scale_y: f64,
        scale_z: f64,
    ) -> f64 {
        let mut freq = 1.0_f64;
        let mut acc = 0.0_f64;
        for octave in self.octaves.iter().rev() {
            if let Some(lattice) = octave {
                acc += lattice.sample_beta_3d_f64(
                    x * scale_x * freq,
                    y * scale_y * freq,
                    z * scale_z * freq,
                ) / freq;
            }
            freq /= 2.0;
        }
        acc
    }

    /// The bulk 3D fill Beta's terrain density grid is built from. Unlike
    /// [`Self::sample_xyz_beta`] this keeps the y lattice cache across the
    /// column, which is what the reference does.
    #[allow(clippy::too_many_arguments)]
    pub fn fill_3d_bulk(
        &self,
        out: &mut [f64],
        x_start: f64,
        y_start: f64,
        z_start: f64,
        x_size: usize,
        y_size: usize,
        z_size: usize,
        x_scale: f64,
        y_scale: f64,
        z_scale: f64,
    ) {
        let mut freq = 1.0_f64;
        for octave in self.octaves.iter().rev() {
            if let Some(lattice) = octave {
                lattice.fill_3d_bulk_at::<f64>(
                    out,
                    x_start,
                    y_start,
                    z_start,
                    x_size,
                    y_size,
                    z_size,
                    x_scale * freq,
                    y_scale * freq,
                    z_scale * freq,
                    1.0 / freq,
                );
            }
            freq /= 2.0;
        }
    }
}

#[cfg(test)]
mod test {
    use super::BetaOctaveNoise;
    use crate::noise::gradient::{GradientNoise, wrap};
    use crate::noise::perlin::PerlinNoise;
    use mcrs_minecraft_random::legacy::LegacyRandom;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct OctaveFixture {
        rng_seed_after_construction: u64,
    }

    #[derive(Deserialize)]
    struct Seed845Fixture {
        beta_octave_perlin_noise_4_octave: OctaveFixture,
    }

    fn load_fixture() -> Seed845Fixture {
        serde_json::from_str(include_str!("fixtures/seed_845.json")).expect("valid fixture JSON")
    }

    /// The normalized fbm the legacy octave list used to expose directly. Kept
    /// here rather than on the type because only this pin needs it.
    fn normalized_fbm(
        octaves: &[Option<GradientNoise>],
        first_octave: i32,
        amplitude_count: usize,
        x: f64,
        y: f64,
        z: f64,
    ) -> f32 {
        let lacunarity = 2.0f32.powi(first_octave);
        // The persistence counts the declared amplitudes, not the octaves the
        // legacy arm allocates, which is the wider of the two.
        let len = amplitude_count as f32;
        let mut persistence = 2.0f32.powf(len - 1.0) / (2.0f32.powf(len) - 1.0);
        let (mut lx, mut ly, mut lz) = (
            x * lacunarity as f64,
            y * lacunarity as f64,
            z * lacunarity as f64,
        );
        let mut acc = 0.0f32;
        for octave in octaves {
            if let Some(lattice) = octave {
                acc += PerlinNoise::from_gradient(lattice.clone()).sample(wrap(lx), wrap(ly), wrap(lz))
                    * persistence;
            }
            lx *= 2.0;
            ly *= 2.0;
            lz *= 2.0;
            persistence *= 0.5;
        }
        acc
    }

    /// Pins the legacy draw order and the Perlin sample against values captured
    /// from the reference.
    #[test]
    fn legacy_arm_sample() {
        let mut random = LegacyRandom::new(381);
        let octaves = GradientNoise::octaves(&mut random, -6, &[1.0, 1.0], true);

        for (sample, expected) in [
            (normalized_fbm(&octaves, -6, 2, 0.0, 0.0, 0.0), 0.029049821),
            (normalized_fbm(&octaves, -6, 2, 0.5, 4.0, -2.0), -0.0034983754),
            (
                normalized_fbm(&octaves, -6, 2, -204.0, 28.0, 12.0),
                0.19407848,
            ),
        ] {
            assert!(
                (sample - expected).abs() < 1e-6,
                "{sample} deviates from {expected}"
            );
        }
    }

    /// The legacy arm burns 262 ints per skipped octave, so the stream position
    /// after a four-octave build is fixed. A drift here moves every Beta world.
    #[test]
    fn beta_octave_draw_count() {
        let fx = load_fixture().beta_octave_perlin_noise_4_octave;
        let mut rng = LegacyRandom::new(845);
        let _noise = BetaOctaveNoise::new(&mut rng, -3, 4);
        assert_eq!(
            rng.seed, fx.rng_seed_after_construction,
            "RNG seed mismatch after 4-octave Beta construction: got {}, expected {}",
            rng.seed, fx.rng_seed_after_construction
        );
    }
}

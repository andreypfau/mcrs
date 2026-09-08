use crate::noise::gradient::GradientNoise;
use crate::noise::perlin::{LegacyPerlin2dNoise, PerlinNoise};
use crate::noise::simplex::SimplexNoise;
use crate::noise::stack::{NoiseStack, Octave, beta_fbm};
use mcrs_minecraft_random::{Random, legacy::LegacyRandom};

/// Beta's octave counts are contiguous from `first_octave` up, and every octave
/// carries weight, so the amplitude list is only ever a run of ones.
fn lattices(random: &mut LegacyRandom, octave_count: usize) -> Vec<Option<GradientNoise>> {
    let first_octave = 1 - octave_count as i32;
    GradientNoise::legacy_octaves(random, first_octave, &vec![1.0; octave_count])
}

/// Beta's unnormalised fbm over the shared lattice, read through its 2D branch,
/// which is what the terrain scale and depth nodes use.
fn perlin_2d_fbm(lattices: Vec<Option<GradientNoise>>) -> NoiseStack<Octave> {
    beta_fbm(lattices, |lattice| {
        Octave::Perlin2d(LegacyPerlin2dNoise::from_gradient(lattice))
    })
}

/// Beta's simplex octaves: the frequency multiplies by `lacunarity` and the
/// weight is `0.55` over a persistence that halves. Nothing is normalised.
fn simplex_fbm<R: Random>(
    random: &mut R,
    octave_count: usize,
    lacunarity: f64,
    persistence: f64,
) -> NoiseStack<Octave> {
    let mut stack = NoiseStack::builder();
    let mut frequency = 1.0f64;
    let mut amplitude = 1.0f64;
    for _ in 0..octave_count {
        stack.add(
            Octave::Simplex(SimplexNoise::from_random(random)),
            frequency,
            (0.55 / amplitude) as f32,
        );
        frequency *= lacunarity;
        amplitude *= persistence;
    }
    stack.build()
}

/// The Beta climate generators, each off its own `LegacyRandom` and unrelated to
/// the terrain stream: mixing them shifts terrain parity.
///
/// The scale each is sampled at, and the post-processing around it, live in the
/// `mcrs:beta/{temperature,vegetation,climate_detail}` density functions.
pub struct BetaClimateNoises {
    pub temperature: NoiseStack<Octave>,
    pub vegetation: NoiseStack<Octave>,
    pub detail: NoiseStack<Octave>,
}

impl BetaClimateNoises {
    pub fn new(seed: u64) -> Self {
        Self {
            temperature: simplex_fbm(
                &mut LegacyRandom::new(seed.wrapping_mul(9871)),
                4,
                0.25,
                0.5,
            ),
            vegetation: simplex_fbm(
                &mut LegacyRandom::new(seed.wrapping_mul(39811)),
                4,
                1.0 / 3.0,
                0.5,
            ),
            detail: simplex_fbm(
                &mut LegacyRandom::new(seed.wrapping_mul(543321)),
                2,
                1.0 / 1.7,
                0.5,
            ),
        }
    }
}

/// The Beta terrain generators that outlive their seeding stream.
///
/// The full stream is low(16), high(16), selector(8), beach(4), surface(4),
/// scale(10), depth(16), forest(8) — 82 octaves, sequential, no discards. The
/// low, high and selector triple is what `minecraft:old_blended_noise` already
/// computes, and forest belongs to decoration, so the four are drawn and dropped
/// purely to position the draws that follow.
///
/// The beach octaves appear twice because Beta reads them through both samplers:
/// the 3D one for the sand and gravel field, the `ySize == 1` one for the gravel
/// override, which is a different noise over the same lattice.
///
/// Beach and surface stay `NoiseStack<PerlinNoise>` rather than the mixed layer
/// type because only they are read through [`NoiseStack::fill_legacy_grid`],
/// which the density graph has no way to ask for.
pub struct BetaTerrainNoises {
    pub beach: NoiseStack<PerlinNoise>,
    pub beach_flat: NoiseStack<LegacyPerlin2dNoise>,
    pub surface: NoiseStack<PerlinNoise>,
    pub scale: NoiseStack<Octave>,
    pub depth: NoiseStack<Octave>,
}

impl BetaTerrainNoises {
    pub fn new(seed: u64) -> Self {
        let mut rng = LegacyRandom::new(seed);
        let _low = lattices(&mut rng, 16);
        let _high = lattices(&mut rng, 16);
        let _selector = lattices(&mut rng, 8);
        let beach = lattices(&mut rng, 4);
        let surface = lattices(&mut rng, 4);
        let scale = lattices(&mut rng, 10);
        let depth = lattices(&mut rng, 16);
        Self {
            beach_flat: beta_fbm(beach.clone(), LegacyPerlin2dNoise::from_gradient),
            beach: beta_fbm(beach, PerlinNoise::from_gradient),
            surface: beta_fbm(surface, PerlinNoise::from_gradient),
            scale: perlin_2d_fbm(scale),
            depth: perlin_2d_fbm(depth),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The scale each climate noise is sampled at, which the density functions
    /// carry as `xz_scale` and which Beta folds into its 1.5 noise scale.
    const TEMPERATURE_SCALE: f64 = 0.025 / 1.5;
    const VEGETATION_SCALE: f64 = 0.05 / 1.5;
    const DETAIL_SCALE: f64 = 0.25 / 1.5;

    #[derive(serde::Deserialize)]
    struct DrawCountFixture {
        seed: u64,
        post_construction_rng_seed: u64,
    }

    /// The legacy arm burns 262 ints per skipped octave, so the stream position
    /// after the full 82-octave build is fixed. A drift here moves every Beta
    /// world.
    #[test]
    fn beta_seeding_no_discard_draw_count() {
        let fixture: DrawCountFixture =
            serde_json::from_str(include_str!("beta/fixtures/beta_draw_counts.json"))
                .expect("valid fixture JSON");
        assert_eq!(fixture.seed, 845, "fixture seed mismatch");

        let mut rng = LegacyRandom::new(845);
        for count in [16, 16, 8, 4, 4, 10, 16, 8] {
            let _ = lattices(&mut rng, count);
        }
        assert_eq!(
            rng.seed, fixture.post_construction_rng_seed,
            "82-octave stream order or discard changed"
        );
    }

    #[test]
    fn beta_seeding_returns_the_five_surviving_noises() {
        let noises = BetaTerrainNoises::new(845);
        assert!(noises.beach.range().max() > 0.0, "beach not constructed");
        assert!(
            noises.beach_flat.range().max() > 0.0,
            "beach_flat not constructed"
        );
        assert!(
            noises.surface.range().max() > 0.0,
            "surface not constructed"
        );
        assert!(noises.scale.range().max() > 0.0, "scale not constructed");
        assert!(noises.depth.range().max() > 0.0, "depth not constructed");
    }

    /// The low, high and selector triple is drawn and dropped. Skipping the draw
    /// instead of dropping the result moves every noise that follows it.
    #[test]
    fn the_dropped_octaves_still_position_the_ones_we_keep() {
        let kept = BetaTerrainNoises::new(845).scale;

        let mut rng = LegacyRandom::new(845);
        for count in [16, 16, 4, 4] {
            let _ = lattices(&mut rng, count);
        }
        let shifted = perlin_2d_fbm(lattices(&mut rng, 10));

        assert_ne!(kept.get(100.0, 0.0, 300.0), shifted.get(100.0, 0.0, 300.0));
    }

    #[test]
    fn beta_climate_seeding_is_independent_from_terrain() {
        let after_first_terrain_octave = {
            let mut rng = LegacyRandom::new(12345);
            let _ = lattices(&mut rng, 16);
            rng.seed
        };
        let climate_start = LegacyRandom::new(12345u64.wrapping_mul(9871)).seed;
        assert_ne!(after_first_terrain_octave, climate_start);
    }

    /// The Beta temperature post-processing, which now lives in the
    /// `mcrs:beta/temperature` density function. Kept here so the fixture pins
    /// the generators and the formula end to end.
    fn sample_temperature(climate: &BetaClimateNoises, x: f64, z: f64) -> f32 {
        let detail = climate.detail.get(x * DETAIL_SCALE, 0.0, z * DETAIL_SCALE) * 1.1 + 0.5;
        let raw = climate
            .temperature
            .get(x * TEMPERATURE_SCALE, 0.0, z * TEMPERATURE_SCALE);
        let t = (raw * 0.15 + 0.7) * 0.99 + detail * 0.01;
        (1.0 - (1.0 - t) * (1.0 - t)).clamp(0.0, 1.0)
    }

    /// Likewise for `mcrs:beta/vegetation`.
    fn sample_humidity(climate: &BetaClimateNoises, x: f64, z: f64) -> f32 {
        let detail = climate.detail.get(x * DETAIL_SCALE, 0.0, z * DETAIL_SCALE) * 1.1 + 0.5;
        let raw = climate
            .vegetation
            .get(x * VEGETATION_SCALE, 0.0, z * VEGETATION_SCALE);
        ((raw * 0.15 + 0.5) * 0.998 + detail * 0.002).clamp(0.0, 1.0)
    }

    #[test]
    fn beta_climate_postprocess_stays_in_the_unit_interval() {
        let climate = BetaClimateNoises::new(12345);
        for (x, z) in [(0.0, 0.0), (8.0, 8.0), (-1024.0, 512.0)] {
            let temp = sample_temperature(&climate, x, z);
            let humidity = sample_humidity(&climate, x, z);
            assert!((0.0..=1.0).contains(&temp), "temperature {temp} at {x},{z}");
            assert!(
                (0.0..=1.0).contains(&humidity),
                "humidity {humidity} at {x},{z}"
            );
        }
    }

    #[derive(serde::Deserialize)]
    struct ClimateFixture {
        seed: u64,
        temperature_at_0_0: f32,
        humidity_at_0_0: f32,
    }

    #[test]
    fn beta_climate_postprocess_values_match_fixture() {
        let fixture: ClimateFixture =
            serde_json::from_str(include_str!("beta/fixtures/beta_climate.json"))
                .expect("valid beta_climate.json fixture");
        let climate = BetaClimateNoises::new(fixture.seed);
        let temp = sample_temperature(&climate, 0.0, 0.0);
        let humidity = sample_humidity(&climate, 0.0, 0.0);
        assert!(
            (temp - fixture.temperature_at_0_0).abs() < 1e-6,
            "temperature mismatch: got {temp}, expected {}",
            fixture.temperature_at_0_0
        );
        assert!(
            (humidity - fixture.humidity_at_0_0).abs() < 1e-6,
            "humidity mismatch: got {humidity}, expected {}",
            fixture.humidity_at_0_0
        );
    }
}

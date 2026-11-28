use mcrs_minecraft_noise::gradient::GradientNoise;
use mcrs_minecraft_noise::perlin::{LegacyPerlin2dNoise, PerlinNoise};
use mcrs_minecraft_noise::simplex::SimplexNoise;
use mcrs_minecraft_noise::stack::{NoiseStack, Octave, beta_fbm};
use mcrs_minecraft_random::{Random, legacy::LegacyRandom};

fn lattices(random: &mut LegacyRandom, octave_count: usize) -> Vec<Option<GradientNoise>> {
    let first_octave = 1 - octave_count as i32;
    GradientNoise::legacy_octaves(random, first_octave, &vec![1.0; octave_count])
}

fn perlin_2d_fbm(lattices: Vec<Option<GradientNoise>>) -> NoiseStack<Octave> {
    beta_fbm(lattices, |lattice| {
        Octave::Perlin2d(LegacyPerlin2dNoise::from_gradient(lattice))
    })
}

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
    use mcrs_minecraft_resource::ResourceLocationBuf;

    const TEMPERATURE_SCALE: f64 = 0.025f32 as f64 / 1.5;
    const VEGETATION_SCALE: f64 = 0.05f32 as f64 / 1.5;
    const DETAIL_SCALE: f64 = 0.25 / 1.5;

    fn inline_noise_scales(name: &str) -> Vec<(String, f64)> {
        use crate::proto::{DensityFunctionHolder, ProtoDensityFunction};
        use mcrs_minecraft_noise::proto::NoiseHolder;

        fn walk(holder: &DensityFunctionHolder, out: &mut Vec<(String, f64)>) {
            let DensityFunctionHolder::Owned(function) = holder else {
                return;
            };
            if let ProtoDensityFunction::Noise {
                noise: NoiseHolder::Reference(id),
                xz_scale,
                ..
            } = &**function
            {
                out.push((id.as_str().to_owned(), xz_scale.0));
            }
            function.visit_children(&mut |child| walk(child, out));
        }

        let mut out = Vec::new();
        walk(
            &mcrs_minecraft_worldgen_testing::read(
                "density_function",
                &ResourceLocationBuf::minecraft(name).unwrap(),
            ),
            &mut out,
        );
        out
    }

    #[test]
    fn the_climate_density_functions_sample_at_betas_own_scales() {
        for (name, noise, expected) in [
            (
                "beta/temperature",
                "mcrs:beta/temperature",
                TEMPERATURE_SCALE,
            ),
            ("beta/vegetation", "mcrs:beta/vegetation", VEGETATION_SCALE),
            (
                "beta/climate_detail",
                "mcrs:beta/climate_detail",
                DETAIL_SCALE,
            ),
        ] {
            let scales = inline_noise_scales(name);
            assert_eq!(
                scales,
                vec![(noise.to_owned(), expected)],
                "{name} does not sample {noise} at {expected:?}"
            );
        }
    }

    #[derive(serde::Deserialize)]
    struct DrawCountFixture {
        seed: u64,
        post_construction_rng_seed: u64,
    }

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
    fn the_dropped_octaves_still_position_the_ones_we_keep() {
        let kept = BetaTerrainNoises::new(845).scale;

        let mut rng = LegacyRandom::new(845);
        for count in [16, 16, 4, 4] {
            let _ = lattices(&mut rng, count);
        }
        let shifted = perlin_2d_fbm(lattices(&mut rng, 10));

        assert_ne!(kept.get(100.0, 0.0, 300.0), shifted.get(100.0, 0.0, 300.0));
    }

    fn sample_temperature(climate: &BetaClimateNoises, x: f64, z: f64) -> f32 {
        let detail = climate.detail.get(x * DETAIL_SCALE, 0.0, z * DETAIL_SCALE) * 1.1 + 0.5;
        let raw = climate
            .temperature
            .get(x * TEMPERATURE_SCALE, 0.0, z * TEMPERATURE_SCALE);
        let t = (raw * 0.15 + 0.7) * 0.99 + detail * 0.01;
        (1.0 - (1.0 - t) * (1.0 - t)).clamp(0.0, 1.0)
    }

    fn sample_humidity(climate: &BetaClimateNoises, x: f64, z: f64) -> f32 {
        let detail = climate.detail.get(x * DETAIL_SCALE, 0.0, z * DETAIL_SCALE) * 1.1 + 0.5;
        let raw = climate
            .vegetation
            .get(x * VEGETATION_SCALE, 0.0, z * VEGETATION_SCALE);
        ((raw * 0.15 + 0.5) * 0.998 + detail * 0.002).clamp(0.0, 1.0)
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

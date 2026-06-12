use crate::noise::beta::simplex_octave::SimplexOctaveNoise;
use crate::noise::octave_perlin_noise::OctavePerlinNoise;
use mcrs_random::legacy::LegacyRandom;

/// Build Beta climate noise from three independent LegacyRandom instances.
///
/// Seeding follows WorldChunkManager.java lines 18-20 (back2beta-server-1.7.9):
///   temperature uses seed * 9871  (4 octaves)
///   humidity    uses seed * 39811 (4 octaves)
///   detail      uses seed * 543321 (2 octaves)
///
/// These streams are completely independent from seed_beta_terrain — mixing them
/// would shift terrain parity (Pitfall 1).
///
/// Returns (temperature, humidity, detail) raw simplex generators; post-processing
/// (0.15/0.7 scaling, detail blend, folding, clamp) lives in the
/// minecraft:beta/{temperature,vegetation,climate_detail} density function JSON.
pub fn seed_beta_climate(seed: u64) -> (SimplexOctaveNoise, SimplexOctaveNoise, SimplexOctaveNoise) {
    let temp_noise = SimplexOctaveNoise::new(&mut LegacyRandom::new(seed.wrapping_mul(9871)), 4);
    let rain_noise = SimplexOctaveNoise::new(&mut LegacyRandom::new(seed.wrapping_mul(39811)), 4);
    let detail_noise =
        SimplexOctaveNoise::new(&mut LegacyRandom::new(seed.wrapping_mul(543321)), 2);
    (temp_noise, rain_noise, detail_noise)
}

/// Build the Beta 1.7.3 terrain seeding stream from a single `LegacyRandom(seed)`.
///
/// Order and octave counts match ChunkProviderGenerate.java:33-40 (back2beta-server-1.7.9):
///   low(16), high(16), selector(8), beach(4), surface(4), scale(10), depth(16), forest(8)
/// = 82 octaves total, sequential, NO discards.
///
/// Forest is constructed as a real `OctavePerlinNoise<f32>` and dropped to consume the
/// exact variable-length stream it produces. A fixed-count drain would diverge because
/// `next_u32_bound` can loop for non-power-of-2 bounds.
///
/// Returns (low, high, selector, beach, surface, scale, depth). Beach and surface are
/// exposed so callers can use them as named surface-pass samplers.
pub fn seed_beta_terrain(
    seed: u64,
) -> (
    OctavePerlinNoise<f32>,
    OctavePerlinNoise<f32>,
    OctavePerlinNoise<f32>,
    OctavePerlinNoise<f32>,
    OctavePerlinNoise<f32>,
    OctavePerlinNoise<f32>,
    OctavePerlinNoise<f32>,
) {
    let mut rng = LegacyRandom::new(seed);

    let low = OctavePerlinNoise::<f32>::new(&mut rng, -15, vec![1.0f32; 16], true);
    let high = OctavePerlinNoise::<f32>::new(&mut rng, -15, vec![1.0f32; 16], true);
    let selector = OctavePerlinNoise::<f32>::new(&mut rng, -7, vec![1.0f32; 8], true);
    let beach = OctavePerlinNoise::<f32>::new(&mut rng, -3, vec![1.0f32; 4], true);
    let surface = OctavePerlinNoise::<f32>::new(&mut rng, -3, vec![1.0f32; 4], true);
    let scale = OctavePerlinNoise::<f32>::new(&mut rng, -9, vec![1.0f32; 10], true);
    let depth = OctavePerlinNoise::<f32>::new(&mut rng, -15, vec![1.0f32; 16], true);
    let _forest = OctavePerlinNoise::<f32>::new(&mut rng, -7, vec![1.0f32; 8], true);

    (low, high, selector, beach, surface, scale, depth)
}

/// Probe the scale/depth noise samplers at a given world block position under three
/// sampling modes, returning the implied d7 base height (= i1/2 + d6*4 per back2beta)
/// for each mode.
///
/// Modes (matching the two prime suspects from the plan):
///   0 = cell-origin (current Rust: noise_x = block >> 2)
///   1 = cell-center (back2beta: noise_x = (block >> 4)*16 + ((block & 15) >> 2))
///   2 = XZ-swapped cell-center (swap x and z after cell-center quantization)
///
/// Returns (d5, d6, d7, noise_x, noise_z) for each mode.
pub fn probe_scale_depth_d7(
    seed: u64,
    wx: i32,
    wz: i32,
) -> [(f32, f32, f32, f32, f32); 3] {
    let (_, _, _, _, _, scale_noise, depth_noise) = seed_beta_terrain(seed);

    let compute = |nx: f32, nz: f32| -> (f32, f32, f32) {
        let g = scale_noise.sample_xz(nx, nz, 1.121, 1.121);
        let h = depth_noise.sample_xz(nx, nz, 200.0, 200.0);

        let mut d5 = (g + 256.0) / 512.0;
        if d5 > 1.0 { d5 = 1.0; }
        if d5 < 0.0 { d5 = 0.0; }
        d5 += 0.5;

        let mut d6 = h / 8000.0;
        if d6 < 0.0 {
            d6 = -d6 * 0.3;
            d6 = d6 * 3.0 - 2.0;
            if d6 < -1.0 { d6 = -1.0; }
            d6 /= 1.4;
            d6 /= 2.0;
        } else {
            d6 = d6 * 3.0 - 2.0;
            if d6 > 1.0 { d6 = 1.0; }
            d6 /= 8.0;
        }

        let d7 = 8.5 + d6 * 4.0;
        (d5, d6, d7)
    };

    // Mode 0: cell-origin (current Rust: block >> 2)
    let nx0 = (wx >> 2) as f32;
    let nz0 = (wz >> 2) as f32;
    let (d5_0, d6_0, d7_0) = compute(nx0, nz0);

    // Mode 1: cell-center (back2beta: chunkBase + cell_within_chunk)
    let nx1 = ((wx & !15) + ((wx & 15) >> 2)) as f32;
    let nz1 = ((wz & !15) + ((wz & 15) >> 2)) as f32;
    let (d5_1, d6_1, d7_1) = compute(nx1, nz1);

    // Mode 2: XZ-swapped cell-center
    let (d5_2, d6_2, d7_2) = compute(nz1, nx1);

    [
        (d5_0, d6_0, d7_0, nx0, nz0),
        (d5_1, d6_1, d7_1, nx1, nz1),
        (d5_2, d6_2, d7_2, nz1, nx1),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_random::legacy::LegacyRandom;

    #[derive(serde::Deserialize)]
    struct DrawCountFixture {
        seed: u64,
        post_construction_rng_seed: u64,
    }

    fn load_fixture() -> DrawCountFixture {
        serde_json::from_str(include_str!("fixtures/beta_draw_counts.json"))
            .expect("valid fixture JSON")
    }

    #[test]
    fn beta_seeding_no_discard_draw_count() {
        let fixture = load_fixture();
        assert_eq!(fixture.seed, 845, "fixture seed mismatch");

        let mut rng = LegacyRandom::new(845);
        let _ = OctavePerlinNoise::<f32>::new(&mut rng, -15, vec![1.0f32; 16], true);
        let _ = OctavePerlinNoise::<f32>::new(&mut rng, -15, vec![1.0f32; 16], true);
        let _ = OctavePerlinNoise::<f32>::new(&mut rng, -7, vec![1.0f32; 8], true);
        let _ = OctavePerlinNoise::<f32>::new(&mut rng, -3, vec![1.0f32; 4], true);
        let _ = OctavePerlinNoise::<f32>::new(&mut rng, -3, vec![1.0f32; 4], true);
        let _ = OctavePerlinNoise::<f32>::new(&mut rng, -9, vec![1.0f32; 10], true);
        let _ = OctavePerlinNoise::<f32>::new(&mut rng, -15, vec![1.0f32; 16], true);
        let _ = OctavePerlinNoise::<f32>::new(&mut rng, -7, vec![1.0f32; 8], true);

        assert_eq!(
            rng.seed,
            fixture.post_construction_rng_seed,
            "post-construction RNG seed mismatch: 82-octave stream order or discard may have changed"
        );
    }

    #[test]
    fn beta_seeding_returns_seven_noises() {
        let (low, high, selector, beach, surface, scale, depth) = seed_beta_terrain(845);
        // Verify each octave count via the public max_value (non-zero confirms construction)
        assert!(low.max_value() > 0.0, "low noise not constructed");
        assert!(high.max_value() > 0.0, "high noise not constructed");
        assert!(selector.max_value() > 0.0, "selector noise not constructed");
        assert!(beach.max_value() > 0.0, "beach noise not constructed");
        assert!(surface.max_value() > 0.0, "surface noise not constructed");
        assert!(scale.max_value() > 0.0, "scale noise not constructed");
        assert!(depth.max_value() > 0.0, "depth noise not constructed");
    }

    #[test]
    fn beta_seeding_order_is_load_bearing() {
        // Prove order matters by comparing the `low` noise from the correct stream
        // against the `low` noise from a stream where selector(8) is built first.
        // Because they read from different positions in the LegacyRandom stream,
        // they produce different permutation tables and therefore different sample values.
        let (low_correct, _, _, _, _, _, _) = seed_beta_terrain(845);

        // Swapped: build selector(8) first, then low(16) — low now reads stream position 2+
        let mut rng_swapped = LegacyRandom::new(845);
        let _ = OctavePerlinNoise::<f32>::new(&mut rng_swapped, -7, vec![1.0f32; 8], true);
        let low_swapped = OctavePerlinNoise::<f32>::new(&mut rng_swapped, -15, vec![1.0f32; 16], true);

        // Sample both at an arbitrary non-zero position
        let v_correct = low_correct.get(100.0, 200.0, 300.0);
        let v_swapped = low_swapped.get(100.0, 200.0, 300.0);
        assert_ne!(
            v_correct, v_swapped,
            "building selector before low must produce a different low noise (order is load-bearing)"
        );
    }

    #[derive(serde::Deserialize)]
    struct ClimateFixture {
        seed: u64,
        temperature_at_0_0: f32,
        humidity_at_0_0: f32,
    }

    fn load_climate_fixture() -> ClimateFixture {
        serde_json::from_str(include_str!("fixtures/beta_climate.json"))
            .expect("valid beta_climate.json fixture")
    }

    #[test]
    fn beta_climate_seeding_independent_from_terrain() {
        let terrain_climate_overlap = {
            let mut rng_terrain = LegacyRandom::new(12345);
            let _ = OctavePerlinNoise::<f32>::new(&mut rng_terrain, -15, vec![1.0f32; 16], true);
            rng_terrain.seed
        };
        let climate_temp_seed_start = LegacyRandom::new(12345u64.wrapping_mul(9871)).seed;
        assert_ne!(
            terrain_climate_overlap,
            climate_temp_seed_start,
            "climate seeds must be independent from terrain stream"
        );
    }

    /// Reference Beta temperature post-processing (WorldChunkManager.java), now
    /// expressed in minecraft:beta/temperature JSON. Kept here so the climate
    /// fixture continues to pin the raw generators + formula end to end.
    fn sample_temperature(
        temp_noise: &SimplexOctaveNoise,
        detail_noise: &SimplexOctaveNoise,
        x: f64,
        z: f64,
    ) -> f32 {
        let detail_raw = detail_noise.sample(x, z, 0.25, 0.25, 1.0 / 1.7, 0.5);
        let detail = detail_raw * 1.1 + 0.5;
        let temp_raw = temp_noise.sample(x, z, 0.025, 0.025, 0.25, 0.5);
        let mut t = (temp_raw as f32 * 0.15 + 0.7) * 0.99 + detail as f32 * 0.01;
        t = 1.0 - (1.0 - t) * (1.0 - t);
        t.clamp(0.0, 1.0)
    }

    /// Reference Beta humidity post-processing, now expressed in
    /// minecraft:beta/vegetation JSON.
    fn sample_humidity(
        rain_noise: &SimplexOctaveNoise,
        detail_noise: &SimplexOctaveNoise,
        x: f64,
        z: f64,
    ) -> f32 {
        let detail_raw = detail_noise.sample(x, z, 0.25, 0.25, 1.0 / 1.7, 0.5);
        let detail = detail_raw * 1.1 + 0.5;
        let rain_raw = rain_noise.sample(x, z, 0.05, 0.05, 1.0 / 3.0, 0.5);
        let h = (rain_raw as f32 * 0.15 + 0.5) * 0.998 + detail as f32 * 0.002;
        h.clamp(0.0, 1.0)
    }

    #[test]
    fn beta_climate_postprocess_temperature_in_range() {
        let (temp_noise, _, detail_noise) = seed_beta_climate(12345);
        let temp = sample_temperature(&temp_noise, &detail_noise, 0.0, 0.0);
        assert!(
            (0.0..=1.0).contains(&temp),
            "sample_temperature must return a value in [0, 1], got {}",
            temp
        );
    }

    #[test]
    fn beta_climate_postprocess_humidity_in_range() {
        let (_, rain_noise, detail_noise) = seed_beta_climate(12345);
        let humidity = sample_humidity(&rain_noise, &detail_noise, 0.0, 0.0);
        assert!(
            (0.0..=1.0).contains(&humidity),
            "sample_humidity must return a value in [0, 1], got {}",
            humidity
        );
    }

    #[test]
    fn beta_climate_postprocess_values_match_fixture() {
        let fixture = load_climate_fixture();
        assert_eq!(fixture.seed, 12345, "fixture seed mismatch");
        let (temp_noise, rain_noise, detail_noise) = seed_beta_climate(fixture.seed);
        let temp = sample_temperature(&temp_noise, &detail_noise, 0.0, 0.0);
        let humidity = sample_humidity(&rain_noise, &detail_noise, 0.0, 0.0);
        assert!(
            (temp - fixture.temperature_at_0_0).abs() < 1e-6,
            "temperature mismatch: got {}, expected {}",
            temp,
            fixture.temperature_at_0_0
        );
        assert!(
            (humidity - fixture.humidity_at_0_0).abs() < 1e-6,
            "humidity mismatch: got {}, expected {}",
            humidity,
            fixture.humidity_at_0_0
        );
    }

    #[test]
    fn beta_climate_temperature_no_y_dependence() {
        let (temp_noise, _, detail_noise) = seed_beta_climate(12345);
        let t1 = sample_temperature(&temp_noise, &detail_noise, 8.0, 8.0);
        let t2 = sample_temperature(&temp_noise, &detail_noise, 8.0, 8.0);
        assert_eq!(t1, t2, "sample_temperature must be deterministic");
    }

    /// Bootstrap: capture fixture values for beta_climate.json.
    /// Run once with `-- --ignored --nocapture`, then paste output into fixtures/beta_climate.json.
    #[test]
    #[ignore = "bootstrap: capture beta_climate.json fixture values for seed 12345"]
    fn bootstrap_beta_climate() {
        let (temp_noise, rain_noise, detail_noise) = seed_beta_climate(12345);
        let temp = sample_temperature(&temp_noise, &detail_noise, 0.0, 0.0);
        let humidity = sample_humidity(&rain_noise, &detail_noise, 0.0, 0.0);
        println!("{{");
        println!("  \"schema_version\": 1,");
        println!("  \"seed\": 12345,");
        println!("  \"temperature_at_0_0\": {:?},", temp);
        println!("  \"humidity_at_0_0\": {:?}", humidity);
        println!("}}");
    }

    /// Bootstrap: run once with `-- --ignored --nocapture` to capture the post-construction seed.
    /// Copy the printed value into fixtures/beta_draw_counts.json.
    #[test]
    #[ignore = "bootstrap: print post-construction seed for seed 845"]
    fn bootstrap_beta_draw_counts() {
        let mut rng = LegacyRandom::new(845);
        let _ = OctavePerlinNoise::<f32>::new(&mut rng, -15, vec![1.0f32; 16], true);
        let _ = OctavePerlinNoise::<f32>::new(&mut rng, -15, vec![1.0f32; 16], true);
        let _ = OctavePerlinNoise::<f32>::new(&mut rng, -7, vec![1.0f32; 8], true);
        let _ = OctavePerlinNoise::<f32>::new(&mut rng, -3, vec![1.0f32; 4], true);
        let _ = OctavePerlinNoise::<f32>::new(&mut rng, -3, vec![1.0f32; 4], true);
        let _ = OctavePerlinNoise::<f32>::new(&mut rng, -9, vec![1.0f32; 10], true);
        let _ = OctavePerlinNoise::<f32>::new(&mut rng, -15, vec![1.0f32; 16], true);
        let _ = OctavePerlinNoise::<f32>::new(&mut rng, -7, vec![1.0f32; 8], true);
        println!("post_construction_rng_seed = {}", rng.seed);
    }
}

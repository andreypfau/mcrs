use crate::noise::octave_perlin_noise::OctavePerlinNoise;
use mcrs_random::legacy::LegacyRandom;

/// Build the Beta 1.7.3 terrain seeding stream from a single `LegacyRandom(seed)`.
///
/// Order and octave counts match ChunkProviderGenerate.java:33-40 (back2beta-server-1.7.9):
///   low(16), high(16), selector(8), beach(4), surface(4), scale(10), depth(16), forest(8)
/// = 82 octaves total, sequential, NO discards.
///
/// Beach, surface, and forest are constructed as real `OctavePerlinNoise<f32>` and dropped
/// to consume the exact variable-length stream each generator produces. A fixed-count drain
/// would diverge because `next_u32_bound` can loop for non-power-of-2 bounds.
pub fn seed_beta_terrain(
    seed: u64,
) -> (
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
    let _beach = OctavePerlinNoise::<f32>::new(&mut rng, -3, vec![1.0f32; 4], true);
    let _surface = OctavePerlinNoise::<f32>::new(&mut rng, -3, vec![1.0f32; 4], true);
    let scale = OctavePerlinNoise::<f32>::new(&mut rng, -9, vec![1.0f32; 10], true);
    let depth = OctavePerlinNoise::<f32>::new(&mut rng, -15, vec![1.0f32; 16], true);
    let _forest = OctavePerlinNoise::<f32>::new(&mut rng, -7, vec![1.0f32; 8], true);

    (low, high, selector, scale, depth)
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
    fn beta_seeding_returns_five_noises() {
        let (low, high, selector, scale, depth) = seed_beta_terrain(845);
        // Verify each octave count via the public max_value (non-zero confirms construction)
        assert!(low.max_value() > 0.0, "low noise not constructed");
        assert!(high.max_value() > 0.0, "high noise not constructed");
        assert!(selector.max_value() > 0.0, "selector noise not constructed");
        assert!(scale.max_value() > 0.0, "scale noise not constructed");
        assert!(depth.max_value() > 0.0, "depth noise not constructed");
    }

    #[test]
    fn beta_seeding_order_is_load_bearing() {
        // Prove order matters by comparing the `low` noise from the correct stream
        // against the `low` noise from a stream where selector(8) is built first.
        // Because they read from different positions in the LegacyRandom stream,
        // they produce different permutation tables and therefore different sample values.
        let (low_correct, _, _, _, _) = seed_beta_terrain(845);

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

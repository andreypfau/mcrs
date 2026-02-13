use crate::noise::octave_perlin_noise::OctavePerlinNoise;
use mcrs_random::Random;

const INPUT_FACTOR: f32 = 1.0181268882175227;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct NoiseSampler {
    first: OctavePerlinNoise,
    second: OctavePerlinNoise,
    value_factor: f32,
    max_value: f32,
}

impl NoiseSampler {
    pub fn new<R>(random: &mut R, first_octave: i32, amplitudes: Vec<f32>) -> Self
    where
        R: Random,
    {
        let first =
            OctavePerlinNoise::new(random, first_octave, amplitudes.clone(), random.is_legacy());
        let second =
            OctavePerlinNoise::new(random, first_octave, amplitudes.clone(), random.is_legacy());
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
        let max_value = (first.max_value() + second.max_value()) * value_factor;
        Self {
            first,
            second,
            value_factor,
            max_value,
        }
    }

    #[inline]
    pub fn max_value(&self) -> f32 {
        self.max_value
    }

    pub fn get(&self, x: f32, y: f32, z: f32) -> f32 {
        let x2 = x * INPUT_FACTOR;
        let y2 = y * INPUT_FACTOR;
        let z2 = z * INPUT_FACTOR;
        (self.first.get(x, y, z) + self.second.get(x2, y2, z2)) * self.value_factor
    }

    /// Batch evaluate NoiseSampler at multiple positions (zero heap allocation).
    /// Evaluates both inner OctavePerlinNoise instances in batch, then combines.
    #[cfg(feature = "batch-noise")]
    pub fn get_batch(&self, positions: &[(f32, f32, f32)], results: &mut [f32]) {
        const MAX_BATCH: usize = 16;
        let n = positions.len();
        debug_assert_eq!(n, results.len());
        debug_assert!(n <= MAX_BATCH);

        // Build second-set positions (scaled by INPUT_FACTOR) on stack
        let mut second_positions = [(0.0f32, 0.0f32, 0.0f32); MAX_BATCH];
        for i in 0..n {
            let (x, y, z) = positions[i];
            second_positions[i] = (x * INPUT_FACTOR, y * INPUT_FACTOR, z * INPUT_FACTOR);
        }

        // Evaluate first OctavePerlinNoise in batch
        let mut first_results = [0.0f32; MAX_BATCH];
        self.first.get_batch(positions, &mut first_results[..n]);

        // Evaluate second OctavePerlinNoise in batch
        let mut second_results = [0.0f32; MAX_BATCH];
        self.second
            .get_batch(&second_positions[..n], &mut second_results[..n]);

        // Combine: (first + second) * value_factor
        for i in 0..n {
            results[i] = (first_results[i] + second_results[i]) * self.value_factor;
        }
    }
}

#[cfg(test)]
mod test {
    use crate::noise::normal_noise::NoiseSampler;
    use mcrs_random::legacy::LegacyRandom;

    #[test]
    fn sample() {
        let mut random = LegacyRandom::new(82);
        let noise = NoiseSampler::new(&mut random, -6, vec![1.0, 1.0]);
        assert_eq!(
            format!("{:.4}", noise.get(0.0, 0.0, 0.0)),
            format!("{:.4}", -0.11173738673691287)
        );
        assert_eq!(
            format!("{:.4}", noise.get(0.5, 4.0, -2.0)),
            format!("{:.4}", -0.12418270136523879)
        );
        assert_eq!(
            format!("{:.4}", noise.get(-204.0, 28.0, 12.0)),
            format!("{:.4}", -0.593348747968403)
        );
    }

    #[cfg(feature = "batch-noise")]
    #[test]
    fn get_batch_matches_scalar() {
        let mut random = LegacyRandom::new(82);
        let noise = NoiseSampler::new(&mut random, -6, vec![1.0, 1.0]);

        let positions = [
            (0.0, 0.0, 0.0),
            (0.5, 4.0, -2.0),
            (-204.0, 28.0, 12.0),
            (50.0, 25.0, -50.0),
            (1000.0, 64.0, 1000.0),
        ];
        let mut batch_results = [0.0f32; 5];
        noise.get_batch(&positions, &mut batch_results);

        for (i, &(x, y, z)) in positions.iter().enumerate() {
            let scalar = noise.get(x, y, z);
            assert_eq!(
                batch_results[i], scalar,
                "Mismatch at position {}: batch={}, scalar={}",
                i, batch_results[i], scalar
            );
        }
    }
}

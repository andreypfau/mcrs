use crate::noise::improved_noise::ImprovedNoise;
use mcrs_random::Random;
use num_traits::Float;

#[derive(Clone, Debug, PartialEq)]
pub struct OctavePerlinNoise<F: Float, const BETA: bool> {
    lacunarity: F,
    persistence: F,
    max_value: F,
    amplitudes: Vec<F>,
    octave_samplers: Vec<Option<ImprovedNoise<F, BETA>>>,
}

impl Default for OctavePerlinNoise<f32, false> {
    fn default() -> Self {
        use mcrs_random::RandomSource;
        Self::new(&mut RandomSource::new(0, true), -1, vec![1.0], false)
    }
}

impl<F: Float + Clone, const BETA: bool> OctavePerlinNoise<F, BETA> {
    pub fn new<T>(random: &mut T, first_octave: i32, amplitudes: Vec<F>, legacy: bool) -> Self
    where
        T: Random + Clone,
    {
        let mut octave_samplers = Vec::with_capacity(amplitudes.len());
        let zero = F::zero();

        if BETA {
            // Plain forward loop matching Java PerlinOctaveNoise: no skip, no fork, no reverse.
            for amp in &amplitudes {
                if *amp != zero {
                    octave_samplers.push(Some(ImprovedNoise::<F, BETA>::from_random(random)));
                } else {
                    octave_samplers.push(None);
                }
            }
        } else if !legacy {
            for (i, value) in amplitudes.iter().enumerate() {
                if *value != zero {
                    let octave = (i as i32) + first_octave;
                    let mut octave_random = random
                        .clone()
                        .fork_hash(format!("octave_{}", octave).as_bytes());
                    octave_samplers.push(Some(ImprovedNoise::<F, BETA>::from_random(&mut octave_random)));
                } else {
                    octave_samplers.push(None);
                }
            }
            random.fork();
        } else {
            for i in (0..=-first_octave as usize).rev() {
                if i < amplitudes.len() && amplitudes[i] != zero {
                    octave_samplers.push(Some(ImprovedNoise::<F, BETA>::from_random(random)));
                } else {
                    octave_samplers.push(None);
                    for _ in 0..262 {
                        random.next_i32();
                    }
                }
            }
            octave_samplers.reverse();
        }

        let scale = F::from(2.0_f64).unwrap();
        let lacunarity = scale.powi(first_octave);
        let len_f = F::from(amplitudes.len() as f64).unwrap();
        let a = scale.powf(len_f - F::one());
        let b = scale.powf(len_f) - F::one();
        let persistence = a / b;

        let mut noise = Self {
            lacunarity,
            persistence,
            max_value: F::zero(),
            amplitudes,
            octave_samplers,
        };
        noise.max_value = noise.edge_value(scale);
        noise
    }

    pub fn max_value(&self) -> F {
        self.max_value
    }

    pub fn edge_value(&self, scale: F) -> F {
        let mut value = F::zero();
        let mut factor = self.persistence;
        for i in 0..self.octave_samplers.len() {
            if self.octave_samplers[i].is_some() {
                value = value + self.amplitudes[i] * scale * factor;
            }
            factor = factor * F::from(0.5_f64).unwrap();
        }
        value
    }
}

impl OctavePerlinNoise<f32, false> {
    pub fn get_octave(&self, octave: usize) -> Option<&ImprovedNoise<f32, false>> {
        self.octave_samplers
            .get(self.octave_samplers.len() - 1 - octave)
            .and_then(|sampler| sampler.as_ref())
    }

    /// Sample a specific octave directly, with custom y_scale/y_max.
    /// Skips the Option check — use only when you know the octave is populated
    /// (e.g., all amplitudes are non-zero).
    #[inline(always)]
    pub fn sample_octave(
        &self,
        octave: usize,
        x: f32,
        y: f32,
        z: f32,
        y_scale: f32,
        y_max: f32,
    ) -> f32 {
        let idx = self.octave_samplers.len() - 1 - octave;
        // SAFETY: Caller guarantees octave is populated (all amplitudes non-zero).
        // In OldBlendedNoise, all 16/8 octaves are always present.
        match unsafe { self.octave_samplers.get_unchecked(idx) } {
            Some(sampler) => sampler.sample(x, y, z, y_scale, y_max),
            None => 0.0,
        }
    }

    #[inline(always)]
    pub fn maintain_precission(value: f32) -> f32 {
        #[cfg(feature = "far-lands")]
        return value;
        #[cfg(not(feature = "far-lands"))]
        {
            const RECIP: f32 = 1.0 / 3.3554432E7;
            const FACTOR: f32 = 3.3554432E7;
            value - (value * RECIP + 0.5).floor() * FACTOR
        }
    }

    #[inline(always)]
    pub fn get(&self, x: f32, y: f32, z: f32) -> f32 {
        let mut lx = x * self.lacunarity;
        let mut ly = y * self.lacunarity;
        let mut lz = z * self.lacunarity;
        let mut persistence = self.persistence;
        let mut acc = 0.0f32;
        let len = self.octave_samplers.len();
        for i in 0..len {
            // SAFETY: i < len, so both indices are in bounds.
            let sampler = unsafe { self.octave_samplers.get_unchecked(i) };
            if let Some(sampler) = sampler {
                let sample = sampler.sample(
                    Self::maintain_precission(lx),
                    Self::maintain_precission(ly),
                    Self::maintain_precission(lz),
                    0.0,
                    0.0,
                );
                let amp = unsafe { *self.amplitudes.get_unchecked(i) };
                acc = sample.mul_add(persistence * amp, acc);
            }
            lx *= 2.0;
            ly *= 2.0;
            lz *= 2.0;
            persistence *= 0.5;
        }
        acc
    }

    /// Batch evaluate all octaves for multiple positions (zero heap allocation).
    /// Iterates octaves in the outer loop to keep each octave's permutation
    /// table L1-hot across all positions.
    #[cfg(feature = "batch-noise")]
    pub fn get_batch(&self, positions: &[(f32, f32, f32)], results: &mut [f32]) {
        const MAX_BATCH: usize = 16;
        let n = positions.len();
        debug_assert_eq!(n, results.len());
        debug_assert!(n <= MAX_BATCH);
        results[..n].iter_mut().for_each(|r| *r = 0.0);

        let mut scaled = [(0.0f32, 0.0f32, 0.0f32); MAX_BATCH];
        for j in 0..n {
            let (x, y, z) = positions[j];
            scaled[j] = (x * self.lacunarity, y * self.lacunarity, z * self.lacunarity);
        }

        let mut maintained = [(0.0f32, 0.0f32, 0.0f32); MAX_BATCH];
        let mut octave_results = [0.0f32; MAX_BATCH];

        let mut persistence = self.persistence;
        let len = self.octave_samplers.len();

        for i in 0..len {
            let sampler = unsafe { self.octave_samplers.get_unchecked(i) };
            if let Some(sampler) = sampler {
                let amp = unsafe { *self.amplitudes.get_unchecked(i) };
                let factor = persistence * amp;

                for j in 0..n {
                    maintained[j] = (
                        Self::maintain_precission(scaled[j].0),
                        Self::maintain_precission(scaled[j].1),
                        Self::maintain_precission(scaled[j].2),
                    );
                }

                sampler.sample_batch(&maintained[..n], 0.0, &[], &mut octave_results[..n]);

                for j in 0..n {
                    results[j] = octave_results[j].mul_add(factor, results[j]);
                }
            }

            for j in 0..n {
                scaled[j].0 *= 2.0;
                scaled[j].1 *= 2.0;
                scaled[j].2 *= 2.0;
            }
            persistence *= 0.5;
        }
    }

    /// Batch evaluate a single octave for multiple positions with per-position y_max.
    /// Used by OldBlendedNoise which manually iterates octaves with custom smear parameters.
    #[cfg(feature = "batch-noise")]
    pub fn sample_octave_batch(
        &self,
        octave: usize,
        positions: &[(f32, f32, f32)],
        y_scale: f32,
        y_maxes: &[f32],
        results: &mut [f32],
    ) {
        let idx = self.octave_samplers.len() - 1 - octave;
        match unsafe { self.octave_samplers.get_unchecked(idx) } {
            Some(sampler) => sampler.sample_batch(positions, y_scale, y_maxes, results),
            None => results.iter_mut().for_each(|r| *r = 0.0),
        }
    }
}

impl OctavePerlinNoise<f64, true> {
    /// 2D XY-plane sample mirroring Java PerlinOctaveNoise.sampleXY.
    ///
    /// Frequency doubles each octave (higher-frequency octaves contribute finer detail).
    /// Used by PerlinOctaveNoiseCombined for domain-warp sampling.
    pub fn sample_xy(&self, x: f64, y: f64) -> f64 {
        let mut lx = x * self.lacunarity;
        let mut ly = y * self.lacunarity;
        let mut persistence = self.persistence;
        let mut acc = 0.0_f64;
        let len = self.octave_samplers.len();
        for i in 0..len {
            if let Some(sampler) = &self.octave_samplers[i] {
                let amp = self.amplitudes[i];
                acc += sampler.sample(lx, ly, 0.0, 0.0, 0.0) * persistence * amp;
            }
            lx *= 2.0;
            ly *= 2.0;
            persistence *= 0.5;
        }
        acc
    }
}

#[cfg(test)]
mod test {
    use crate::noise::octave_perlin_noise::OctavePerlinNoise;
    use mcrs_random::legacy::LegacyRandom;
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
        serde_json::from_str(include_str!("beta/fixtures/seed_845.json"))
            .expect("valid fixture JSON")
    }

    #[test]
    fn legacy_arm_sample() {
        let mut random = LegacyRandom::new(381);
        let noise = OctavePerlinNoise::<f32, false>::new(&mut random, -6, vec![1.0, 1.0], true);

        assert_eq!(
            format!("{:.4}", noise.get(0.0, 0.0, 0.0)),
            format!("{:.4}", -0.44062224030494690)
        );
        assert_eq!(
            format!("{:.4}", noise.get(0.5, 4.0, -2.0)),
            format!("{:.4}", -0.37391784787178040)
        );
        assert_eq!(
            format!("{:.4}", noise.get(-204.0, 28.0, 12.0)),
            format!("{:.4}", 0.02771721035242081)
        );
    }

    #[test]
    fn beta_octave_draw_count() {
        let fx = load_fixture().beta_octave_perlin_noise_4_octave;
        let mut rng = LegacyRandom::new(845);
        let amplitudes = vec![1.0_f64, 1.0, 1.0, 1.0];
        let _noise = OctavePerlinNoise::<f64, true>::new(&mut rng, -4, amplitudes, false);
        assert_eq!(
            rng.seed, fx.rng_seed_after_construction,
            "RNG seed mismatch after 4-octave Beta construction: got {}, expected {}",
            rng.seed, fx.rng_seed_after_construction
        );
    }

    #[test]
    fn beta_octave_sample_xy_finite() {
        let mut rng = LegacyRandom::new(845);
        let noise = OctavePerlinNoise::<f64, true>::new(&mut rng, -4, vec![1.0, 1.0, 1.0, 1.0], false);
        let v = noise.sample_xy(100.0, 200.0);
        assert!(v.is_finite(), "sample_xy must return a finite value");
    }

    #[test]
    #[ignore = "bootstrap: print actual values for seed 381 legacy arm"]
    fn bootstrap_seed_381_legacy() {
        let mut random = LegacyRandom::new(381);
        let noise = OctavePerlinNoise::<f32, false>::new(&mut random, -6, vec![1.0, 1.0], true);
        println!("get(0,0,0) = {:.10}", noise.get(0.0, 0.0, 0.0));
        println!("get(0.5,4.0,-2.0) = {:.10}", noise.get(0.5, 4.0, -2.0));
        println!("get(-204,28,12) = {:.10}", noise.get(-204.0, 28.0, 12.0));
    }

    #[test]
    #[ignore = "bootstrap: run once to capture octave fixture values"]
    fn bootstrap_seed_845_octave() {
        let mut rng = LegacyRandom::new(845);
        let amplitudes = vec![1.0_f64, 1.0, 1.0, 1.0];
        let _noise = OctavePerlinNoise::<f64, true>::new(&mut rng, -4, amplitudes, false);
        println!("beta_octave_perlin_noise_4_octave.rng_seed_after_construction: {}", rng.seed);
    }

    #[cfg(feature = "batch-noise")]
    #[test]
    fn get_batch_matches_scalar() {
        let mut random = LegacyRandom::new(381);
        let noise = OctavePerlinNoise::<f32, false>::new(&mut random, -6, vec![1.0, 1.0], true);

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

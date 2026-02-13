use crate::noise::improved_noise::ImprovedNoise;
use mcrs_random::Random;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct OctavePerlinNoise {
    lacunarity: f32,
    persistence: f32,
    max_value: f32,
    amplitudes: Vec<f32>,
    octave_samplers: Vec<Option<ImprovedNoise>>,
}

impl OctavePerlinNoise {
    pub fn new<T>(random: &mut T, first_octave: i32, amplitudes: Vec<f32>, legacy: bool) -> Self
    where
        T: Random + Clone,
    {
        let mut octave_samplers = Vec::with_capacity(amplitudes.len());

        if !legacy {
            for (i, value) in amplitudes.iter().enumerate() {
                if *value != 0.0 {
                    let octave = (i as i32) + first_octave;
                    let mut octave_random = random
                        .clone()
                        .fork_hash(format!("octave_{}", octave).as_bytes());
                    octave_samplers.push(Some(ImprovedNoise::from_random(&mut octave_random)));
                } else {
                    octave_samplers.push(None);
                }
            }
            random.fork();
        } else {
            for i in (0..=-first_octave as usize).rev() {
                if i < amplitudes.len() && amplitudes[i] != 0.0 {
                    octave_samplers.push(Some(ImprovedNoise::from_random(random)));
                } else {
                    octave_samplers.push(None);
                    for _ in 0..262 {
                        random.next_i32();
                    }
                }
            }
            octave_samplers.reverse();
        }

        let scale = 2.0_f32;
        let lacunarity = scale.powi(first_octave);
        let a = scale.powf(amplitudes.len() as f32 - 1.0);
        let b = scale.powf(amplitudes.len() as f32) - 1.0;
        let persistence = a / b;

        let mut noise = Self {
            lacunarity,
            persistence,
            max_value: 0.0,
            amplitudes,
            octave_samplers,
        };
        noise.max_value = noise.edge_value(scale);
        noise
    }

    pub fn get_octave(&self, octave: usize) -> Option<&ImprovedNoise> {
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

    pub fn max_value(&self) -> f32 {
        self.max_value
    }

    pub fn edge_value(&self, scale: f32) -> f32 {
        let mut value = 0.0;
        let mut factor = self.persistence;
        for i in 0..self.octave_samplers.len() {
            if self.octave_samplers[i].is_some() {
                value += self.amplitudes[i] * scale * factor;
            }
            factor *= 0.5;
        }
        value
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
        // Strength-reduce: scale coordinates directly instead of multiplying
        // by a separate lacunarity variable each iteration.
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

        // Pre-compute scaled positions (initial scale = lacunarity) on stack
        let mut scaled = [(0.0f32, 0.0f32, 0.0f32); MAX_BATCH];
        for j in 0..n {
            let (x, y, z) = positions[j];
            scaled[j] = (x * self.lacunarity, y * self.lacunarity, z * self.lacunarity);
        }

        // Temp buffers on stack
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

#[cfg(test)]
mod test {
    use crate::noise::octave_perlin_noise::OctavePerlinNoise;
    use mcrs_random::legacy::LegacyRandom;

    #[test]
    fn sample() {
        let mut random = LegacyRandom::new(381);
        let noise = OctavePerlinNoise::new(&mut random, -6, vec![1.0, 1.0], true);

        assert_eq!(
            format!("{:.4}", noise.get(0.0, 0.0, 0.0)),
            format!("{:.4}", 0.02904968471563733)
        );
        assert_eq!(
            format!("{:.4}", noise.get(0.5, 4.0, -2.0)),
            format!("{:.4}", -0.003498819899307167)
        );
        assert_eq!(
            format!("{:.4}", noise.get(-204.0, 28.0, 12.0)),
            format!("{:.4}", 0.19407799903721645)
        );
    }

    #[cfg(feature = "batch-noise")]
    #[test]
    fn get_batch_matches_scalar() {
        let mut random = LegacyRandom::new(381);
        let noise = OctavePerlinNoise::new(&mut random, -6, vec![1.0, 1.0], true);

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

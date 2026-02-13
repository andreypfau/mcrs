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
    #[inline]
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
        match &self.octave_samplers[idx] {
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

    #[inline]
    pub fn maintain_precission(value: f32) -> f32 {
        #[cfg(feature = "far-lands")]
        return value;
        #[cfg(not(feature = "far-lands"))]
        return (value - (value / 3.3554432E7 + 0.5).floor() * 3.3554432E7);
    }

    #[inline]
    pub fn get(&self, x: f32, y: f32, z: f32) -> f32 {
        let mut lacunarity = self.lacunarity;
        let mut persistence = self.persistence;
        let mut acc = 0.0;
        for i in 0..self.octave_samplers.len() {
            if let Some(sampler) = &self.octave_samplers[i] {
                let sample = sampler.sample(
                    OctavePerlinNoise::maintain_precission(x * lacunarity),
                    OctavePerlinNoise::maintain_precission(y * lacunarity),
                    OctavePerlinNoise::maintain_precission(z * lacunarity),
                    0.0,
                    0.0,
                );
                acc += sample * persistence * self.amplitudes[i];
            }
            lacunarity *= 2.0;
            persistence *= 0.5;
        }
        acc
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
}

use crate::noise::beta::simplex::SimplexNoise;
use mcrs_random::Random;

pub struct SimplexOctaveNoise {
    noises: Vec<SimplexNoise>,
    noise_scale: f64,
}

impl SimplexOctaveNoise {
    pub fn new<T: Random>(random: &mut T, octaves: usize) -> Self {
        let noises = (0..octaves)
            .map(|_| SimplexNoise::from_random(random))
            .collect();
        Self { noises, noise_scale: 1.5 }
    }

    pub fn sample(
        &self,
        x: f64,
        z: f64,
        scale_x: f64,
        scale_z: f64,
        lacunarity: f64,
        persistence: f64,
    ) -> f64 {
        let scale_x = scale_x / self.noise_scale;
        let scale_z = scale_z / self.noise_scale;
        let mut total = 0.0_f64;
        let mut amplitude = 1.0_f64;
        let mut frequency = 1.0_f64;
        for noise in &self.noises {
            total += noise.sample(x, z, scale_x * frequency, scale_z * frequency) * (0.55 / amplitude);
            frequency *= lacunarity;
            amplitude *= persistence;
        }
        total
    }
}

#[cfg(test)]
mod test {
    use super::SimplexOctaveNoise;
    use mcrs_random::legacy::LegacyRandom;

    #[test]
    fn simplex_octave_reachable() {
        let noise = SimplexOctaveNoise::new(&mut LegacyRandom::new(845), 4);
        let v = noise.sample(1.0, 1.0, 1.0, 1.0, 2.0, 0.5);
        assert!(v.is_finite(), "sample must return a finite f64");
    }
}

use crate::noise::beta::BetaOctavePerlinNoise;

pub struct PerlinOctaveNoiseCombined {
    first: BetaOctavePerlinNoise,
    second: BetaOctavePerlinNoise,
}

impl PerlinOctaveNoiseCombined {
    pub fn new(first: BetaOctavePerlinNoise, second: BetaOctavePerlinNoise) -> Self {
        Self { first, second }
    }

    pub fn sample(&self, x: f64, y: f64) -> f64 {
        self.first.sample_xy(x + self.second.sample_xy(x, y), y)
    }
}

#[cfg(test)]
mod test {
    use super::PerlinOctaveNoiseCombined;
    use crate::noise::octave_perlin_noise::OctavePerlinNoise;
    use mcrs_random::legacy::LegacyRandom;

    #[test]
    fn combined_reachable() {
        let mut rng = LegacyRandom::new(845);
        let first = OctavePerlinNoise::<f64>::new(&mut rng, -3, vec![1.0, 1.0, 1.0, 1.0], true);
        let second = OctavePerlinNoise::<f64>::new(&mut rng, -3, vec![1.0, 1.0, 1.0, 1.0], true);
        let combined = PerlinOctaveNoiseCombined::new(first, second);
        let v = combined.sample(0.5, 0.5);
        assert!(v.is_finite(), "sample must return a finite f64");
    }
}

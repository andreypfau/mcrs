use crate::noise::octave_perlin_noise::OctavePerlinNoise;
use mcrs_random::Random;

const INPUT_FACTOR: f64 = 1.0181268882175227;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct NormalNoise {
    first: OctavePerlinNoise,
    second: OctavePerlinNoise,
    value_factor: f64,
    max_value: f64,
}

impl NormalNoise {
    pub fn new<R>(random: &mut R, first_octave: i32, amplitudes: Vec<f64>) -> Self
    where
        R: Random,
    {
        let first =
            OctavePerlinNoise::new(random, first_octave, amplitudes.clone(), random.is_legacy());
        let second =
            OctavePerlinNoise::new(random, first_octave, amplitudes.clone(), random.is_legacy());
        let mut min = f64::INFINITY;
        let mut max = f64::NEG_INFINITY;
        for (i, value) in amplitudes.iter().enumerate() {
            if *value != 0.0 {
                min = min.min(i as f64);
                max = max.max(i as f64);
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
    pub fn max_value(&self) -> f64 {
        self.max_value
    }

    pub fn get(&self, x: f64, y: f64, z: f64) -> f64 {
        let x2 = x * INPUT_FACTOR;
        let y2 = y * INPUT_FACTOR;
        let z2 = z * INPUT_FACTOR;
        (self.first.get(x, y, z) + self.second.get(x2, y2, z2)) * self.value_factor
    }
}

#[cfg(test)]
mod test {
    use crate::noise::normal_noise::NormalNoise;
    use mcrs_random::legacy::LegacyRandom;

    #[test]
    fn sample() {
        let mut random = LegacyRandom::new(82);
        let noise = NormalNoise::new(&mut random, -6, vec![1.0, 1.0]);
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
}

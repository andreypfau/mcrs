use crate::noise::octave_perlin_noise::OctavePerlinNoise;
use mcrs_random::RandomSource;

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BlendedNoise {
    xz_scale: f64,
    y_scale: f64,
    xz_factor: f64,
    y_factor: f64,
    smear_scale_multiplier: f64,
    #[cfg_attr(feature = "serde", serde(skip))]
    xz_multiplier: f64,
    #[cfg_attr(feature = "serde", serde(skip))]
    y_multiplier: f64,
    #[cfg_attr(feature = "serde", serde(skip))]
    max_value: f64,
    #[cfg_attr(feature = "serde", serde(skip))]
    smear: f64,
    #[cfg_attr(feature = "serde", serde(skip))]
    factored_smeared: f64,
    #[cfg_attr(feature = "serde", serde(skip))]
    lower_interpolated_noise: Option<OctavePerlinNoise>,
    #[cfg_attr(feature = "serde", serde(skip))]
    upper_interpolated_noise: Option<OctavePerlinNoise>,
    #[cfg_attr(feature = "serde", serde(skip))]
    interpolated_noise: Option<OctavePerlinNoise>,
}

impl Default for BlendedNoise {
    fn default() -> Self {
        Self::new(&mut RandomSource::new(0, false), 1.0, 1.0, 1.0, 1.0, 1.0)
    }
}

impl BlendedNoise {
    pub fn new(
        random: &mut RandomSource,
        xz_scale: f64,
        y_scale: f64,
        xz_factor: f64,
        y_factor: f64,
        smear_scale_multiplier: f64,
    ) -> Self {
        Self {
            xz_scale,
            y_scale,
            xz_factor,
            y_factor,
            smear_scale_multiplier,
            xz_multiplier: 0.0,
            y_multiplier: 0.0,
            smear: 0.0,
            factored_smeared: 0.0,
            max_value: f64::INFINITY,
            lower_interpolated_noise: None,
            upper_interpolated_noise: None,
            interpolated_noise: None,
        }
        .with_random(random)
    }
}

impl BlendedNoise {
    pub fn with_random(mut self, random: &mut RandomSource) -> Self {
        self.xz_multiplier = 684.412 * self.xz_scale;
        self.y_multiplier = 684.412 * self.y_scale;
        self.smear = self.smear_scale_multiplier * self.y_multiplier;
        self.factored_smeared = self.smear / self.y_factor;
        let lower_interpolated_noise = OctavePerlinNoise::new(random, -15, vec![1.0; 16], true);
        self.max_value = lower_interpolated_noise.edge_value(self.y_multiplier + 2.0);
        self.lower_interpolated_noise = Some(lower_interpolated_noise);
        self.upper_interpolated_noise =
            Some(OctavePerlinNoise::new(random, -15, vec![1.0; 16], true));
        self.interpolated_noise = Some(OctavePerlinNoise::new(random, -7, vec![1.0; 8], true));
        self
    }

    pub fn compute(&self, x: i32, y: i32, z: i32) -> f64 {
        let scaled_x = x as f64 * self.xz_multiplier;
        let scaled_y = y as f64 * self.y_multiplier;
        let scaled_z = z as f64 * self.xz_multiplier;

        let factored_x = scaled_x / self.xz_factor;
        let factored_y = scaled_y / self.y_factor;
        let factored_z = scaled_z / self.xz_factor;

        let mut value = 0.0;
        let mut factor = 1.0;
        for i in 0..8 {
            let noise = self
                .interpolated_noise
                .as_ref()
                .and_then(|n| n.get_octave(i));
            if let Some(noise) = noise {
                let xx = OctavePerlinNoise::maintain_precission(factored_x * factor);
                let yy = OctavePerlinNoise::maintain_precission(factored_y * factor);
                let zz = OctavePerlinNoise::maintain_precission(factored_z * factor);
                value += noise.sample(
                    xx,
                    yy,
                    zz,
                    self.factored_smeared * factor,
                    factored_y * factor,
                ) / factor;
            }
            factor /= 2.0;
        }

        value = (value / 10.0 + 1.0) / 2.0;
        factor = 1.0;
        let less_than_one = value < 1.0;
        let more_than_zero = value > 0.0;
        let mut min = 0.0;
        let mut max = 0.0;
        for i in 0..16 {
            let xx = OctavePerlinNoise::maintain_precission(scaled_x * factor);
            let yy = OctavePerlinNoise::maintain_precission(scaled_y * factor);
            let zz = OctavePerlinNoise::maintain_precission(scaled_z * factor);
            let smears_smear = self.smear * factor;
            if less_than_one {
                let noise = self
                    .lower_interpolated_noise
                    .as_ref()
                    .and_then(|n| n.get_octave(i));
                if let Some(noise) = noise {
                    min += noise.sample(xx, yy, zz, smears_smear, scaled_y * factor) / factor;
                }
            }
            if more_than_zero {
                let noise = self
                    .upper_interpolated_noise
                    .as_ref()
                    .and_then(|n| n.get_octave(i));
                if let Some(noise) = noise {
                    max += noise.sample(xx, yy, zz, smears_smear, scaled_y * factor) / factor;
                }
            }
            factor /= 2.0;
        }

        let start = min / 512.0;
        let end = max / 512.0;
        value = if value < 0.0 {
            start
        } else if value > 1.0 {
            end
        } else {
            value * (end - start) + start
        };
        value / 128.0
    }

    #[inline]
    pub fn min_value(&self) -> f64 {
        -self.max_value
    }

    #[inline]
    pub fn max_value(&self) -> f64 {
        self.max_value
    }
}

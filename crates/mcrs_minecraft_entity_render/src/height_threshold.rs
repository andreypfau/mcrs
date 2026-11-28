use crate::frame_uniform::FULL_RIG_HEIGHT;

const MAX_HEIGHT: f32 = 16384.0;
const HEADROOM: f32 = 0.8;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeightThreshold {
    pixels: f32,
}

impl Default for HeightThreshold {
    fn default() -> Self {
        Self {
            pixels: FULL_RIG_HEIGHT,
        }
    }
}

impl HeightThreshold {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn pixels(self) -> f32 {
        self.pixels
    }

    pub fn update(&mut self, full_rigs: u32, cap: u32) {
        let (count, cap) = (full_rigs as f32, cap.max(1) as f32);
        if count > cap || count < HEADROOM * cap {
            self.pixels = (self.pixels * (count / cap).sqrt()).clamp(FULL_RIG_HEIGHT, MAX_HEIGHT);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_threshold_follows_the_full_rig_count() {
        let mut threshold = HeightThreshold::new();
        threshold.update(300, 75);
        assert_eq!(threshold.pixels(), 24.0, "four times the cap doubles it");
        threshold.update(70, 75);
        assert_eq!(
            threshold.pixels(),
            24.0,
            "between 0.8 and 1 of the cap it holds"
        );
        threshold.update(75 / 4, 75);
        assert!(
            (threshold.pixels() - 12.0).abs() < 0.1,
            "a quarter of the cap halves it: {}",
            threshold.pixels()
        );
        threshold.update(0, 75);
        assert_eq!(
            threshold.pixels(),
            FULL_RIG_HEIGHT,
            "it never falls below 12 px"
        );
    }
}

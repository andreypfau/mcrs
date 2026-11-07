/// The part of the gap to the target speed that the walk speed closes in each tick.
pub const WALK_SMOOTHING: f32 = 0.4;
/// The walk speed that a damage event sets.
pub const HURT_WALK_SPEED: f32 = 1.5;
/// The walk scale of a baby, whose legs swing faster.
pub const BABY_WALK_SCALE: f32 = 3.0;

/// The walk accumulator: the speed, the speed at the start of the latest tick, the position
/// and the scale. The renderer swings the legs with the values of [`Walk::frame`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Walk {
    pub speed: f32,
    pub previous_speed: f32,
    pub position: f32,
    pub scale: f32,
}

impl Default for Walk {
    fn default() -> Self {
        Self {
            speed: 0.0,
            previous_speed: 0.0,
            position: 0.0,
            scale: 1.0,
        }
    }
}

/// The walk values of one frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WalkFrame {
    pub position: f32,
    pub speed: f32,
}

impl Walk {
    /// One tick towards `target` speed, with the walk scale `scale`.
    pub fn advance(&mut self, target: f32, scale: f32) {
        self.previous_speed = self.speed;
        self.speed += (target - self.speed) * WALK_SMOOTHING;
        self.position += self.speed;
        self.scale = scale;
    }

    /// The tick of an entity that is not alive.
    pub fn stop(&mut self) {
        self.speed = 0.0;
        self.previous_speed = 0.0;
        self.position = 0.0;
    }

    pub fn hurt(&mut self) {
        self.speed = HURT_WALK_SPEED;
    }

    /// The values at the partial tick `partial_tick` of an entity that is alive.
    #[must_use]
    pub fn frame(&self, partial_tick: f32) -> WalkFrame {
        WalkFrame {
            position: (self.position - self.speed * (1.0 - partial_tick)) * self.scale,
            speed: (self.previous_speed + partial_tick * (self.speed - self.previous_speed))
                .min(1.0),
        }
    }
}

/// The target walk speed for a tick with the horizontal distance `distance` and the walk target
/// factor `factor` of the type.
#[must_use]
pub fn target_speed(distance: f32, factor: f32) -> f32 {
    (factor * distance).min(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_speed_closes_four_tenths_of_the_gap_each_tick() {
        let mut walk = Walk::default();
        walk.advance(target_speed(0.1, 4.0), 1.0);
        assert!((walk.speed - 0.16).abs() < 1e-6);
        assert!((walk.position - 0.16).abs() < 1e-6);
        walk.advance(target_speed(0.1, 4.0), 1.0);
        assert!((walk.speed - 0.256).abs() < 1e-6);
        assert!((walk.position - 0.416).abs() < 1e-6);
        assert!((walk.previous_speed - 0.16).abs() < 1e-6);
    }

    #[test]
    fn the_target_saturates_at_one() {
        assert_eq!(target_speed(0.3, 4.0), 1.0);
        assert_eq!(target_speed(0.05, 25.0), 1.0);
    }

    #[test]
    fn a_frame_interpolates_and_caps_the_speed() {
        let walk = Walk {
            speed: 1.5,
            previous_speed: 0.5,
            position: 3.0,
            scale: 3.0,
        };
        let frame = walk.frame(0.25);
        assert_eq!(frame.speed, 0.75);
        assert_eq!(frame.position, (3.0 - 1.5 * 0.75) * 3.0);
        assert_eq!(
            walk.frame(0.75).speed,
            1.0,
            "a hurt speed above 1 is capped"
        );
    }
}

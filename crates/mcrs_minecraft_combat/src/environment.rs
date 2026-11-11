use bevy_ecs::component::Component;

/// The air supply of an entity under water (#108).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct AirSupply {
    pub air: i32,
    pub max_air: i32,
}

impl Default for AirSupply {
    fn default() -> Self {
        Self {
            air: 300,
            max_air: 300,
        }
    }
}

impl AirSupply {
    #[must_use]
    pub const fn new(max_air: i32) -> Self {
        Self { air: max_air, max_air }
    }

    /// Advances one tick under water, returning whether to deal drowning damage (2.0 HP).
    pub fn tick_underwater(&mut self) -> bool {
        self.air -= 1;
        if self.air <= -20 {
            self.air = 0;
            true
        } else {
            false
        }
    }

    /// Advances one tick in open air, recovering breath.
    pub fn tick_in_air(&mut self) {
        if self.air < self.max_air {
            self.air = (self.air + 4).min(self.max_air);
        }
    }
}

/// The burning state of an entity (#109).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FireState {
    pub remaining_fire_ticks: i32,
}

impl FireState {
    /// Catches fire for `ticks`.
    pub fn ignite(&mut self, ticks: i32) {
        self.remaining_fire_ticks = self.remaining_fire_ticks.max(ticks);
    }

    /// Extinguishes the fire immediately (e.g. entering water or rain).
    pub fn extinguish(&mut self) {
        self.remaining_fire_ticks = 0;
    }

    /// Advances one tick of burning, returning whether to deal 1.0 fire damage.
    pub fn tick(&mut self) -> bool {
        if self.remaining_fire_ticks > 0 {
            self.remaining_fire_ticks -= 1;
            self.remaining_fire_ticks % 20 == 0
        } else {
            false
        }
    }
}

/// Computes fall damage from fall distance and safe fall distance (DMG-FALL-002).
#[must_use]
pub fn compute_fall_damage(fall_distance: f64, safe_fall_distance: f64, fall_multiplier: f32) -> f32 {
    let excess = fall_distance - safe_fall_distance;
    if excess <= 0.0 {
        0.0
    } else {
        (excess.ceil() as f32 * fall_multiplier).max(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn air_depletes_underwater_and_triggers_drowning() {
        let mut air = AirSupply::new(10);
        for _ in 0..10 {
            assert!(!air.tick_underwater());
        }
        assert_eq!(air.air, 0);

        for _ in 0..19 {
            assert!(!air.tick_underwater());
        }
        // At -20, triggers drowning damage and resets to 0
        assert!(air.tick_underwater());
        assert_eq!(air.air, 0);
    }

    #[test]
    fn fire_deals_damage_every_20_ticks() {
        let mut fire = FireState::default();
        fire.ignite(40);
        let mut damage_count = 0;
        for _ in 0..40 {
            if fire.tick() {
                damage_count += 1;
            }
        }
        assert_eq!(damage_count, 2);
        assert_eq!(fire.remaining_fire_ticks, 0);
    }

    #[test]
    fn fall_damage_calculation() {
        // Fall of 4 blocks with safe distance 3.0 -> ceil(1.0) = 1.0 damage
        assert_eq!(compute_fall_damage(4.0, 3.0, 1.0), 1.0);
        // Fall of 2.5 blocks -> 0.0 damage
        assert_eq!(compute_fall_damage(2.5, 3.0, 1.0), 0.0);
        // Fall of 10.2 blocks with safe 3.0 -> ceil(7.2) = 8.0 damage
        assert_eq!(compute_fall_damage(10.2, 3.0, 1.0), 8.0);
    }
}

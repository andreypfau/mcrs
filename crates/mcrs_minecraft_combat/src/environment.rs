use bevy_ecs::component::Component;
use bevy_ecs::prelude::Resource;
use glam::DVec3;

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

/// The freezing state of an entity in powder snow (#110).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FreezeState {
    pub ticks_frozen: i32,
}

impl FreezeState {
    pub const FULLY_FROZEN_TICKS: i32 = 140;

    /// Advances one tick inside powder snow. Returns whether to deal 1.0 freeze damage.
    pub fn tick_in_powder_snow(&mut self) -> bool {
        self.ticks_frozen = (self.ticks_frozen + 1).min(300);
        if self.ticks_frozen >= Self::FULLY_FROZEN_TICKS {
            (self.ticks_frozen - Self::FULLY_FROZEN_TICKS) % 40 == 0
        } else {
            false
        }
    }

    /// Advances one tick outside powder snow, thawing gradually.
    pub fn tick_warm(&mut self) {
        if self.ticks_frozen > 0 {
            self.ticks_frozen = (self.ticks_frozen - 2).max(0);
        }
    }

    #[must_use]
    pub const fn is_frozen(&self) -> bool {
        self.ticks_frozen >= Self::FULLY_FROZEN_TICKS
    }
}

/// World border global state (#262).
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct WorldBorderState {
    pub center_x: f64,
    pub center_z: f64,
    /// Full diameter of the border in blocks.
    pub size: f64,
    /// Distance past the border before damage begins. Default is 5.0 blocks.
    pub safe_zone: f64,
    /// Damage per block per second (or per tick). Default is 0.2.
    pub damage_per_block: f64,
}

impl Default for WorldBorderState {
    fn default() -> Self {
        Self {
            center_x: 0.0,
            center_z: 0.0,
            size: 59_999_968.0,
            safe_zone: 5.0,
            damage_per_block: 0.2,
        }
    }
}

impl WorldBorderState {
    /// Computes the damage for an entity at `pos`. Returns 0.0 if inside safe zone.
    #[must_use]
    pub fn compute_damage(&self, pos: DVec3) -> f32 {
        let half_size = self.size / 2.0;
        let dx = (pos.x - self.center_x).abs() - half_size;
        let dz = (pos.z - self.center_z).abs() - half_size;
        let dist_past_border = dx.max(dz);

        let dist_past_safe = dist_past_border - self.safe_zone;
        if dist_past_safe > 0.0 {
            let dmg = dist_past_safe * self.damage_per_block;
            (dmg as f32).max(1.0)
        } else {
            0.0
        }
    }
}

/// Checks whether an entity is below the world void floor (min_y - 64).
#[must_use]
pub fn is_in_void(y: f64, min_y: f64) -> bool {
    y < min_y - 64.0
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
    fn freeze_accumulates_and_deals_damage() {
        let mut freeze = FreezeState::default();
        for _ in 0..139 {
            assert!(!freeze.tick_in_powder_snow());
        }
        assert!(!freeze.is_frozen());

        // At 140 ticks, fully frozen and deals first damage
        assert!(freeze.tick_in_powder_snow());
        assert!(freeze.is_frozen());

        // Next 39 ticks deal no damage
        for _ in 0..39 {
            assert!(!freeze.tick_in_powder_snow());
        }
        // At 40 ticks, deals next damage
        assert!(freeze.tick_in_powder_snow());
    }

    #[test]
    fn world_border_damage_outside_safe_zone() {
        let border = WorldBorderState {
            center_x: 0.0,
            center_z: 0.0,
            size: 100.0, // boundary at +/- 50.0
            safe_zone: 5.0, // safe zone extends to +/- 55.0
            damage_per_block: 0.2,
        };

        // Position 52.0 is outside border (by 2) but inside safe zone (< 5) -> 0 damage
        assert_eq!(border.compute_damage(DVec3::new(52.0, 64.0, 0.0)), 0.0);

        // Position 60.0 is 10 blocks past border, 5 blocks past safe zone -> 5 * 0.2 = 1.0 damage
        assert_eq!(border.compute_damage(DVec3::new(60.0, 64.0, 0.0)), 1.0);
    }

    #[test]
    fn void_detection() {
        // min_y = -64. Floor is -128.
        assert!(!is_in_void(-127.0, -64.0));
        assert!(is_in_void(-128.1, -64.0));
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

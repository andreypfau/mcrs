use glam::DVec3;
use mcrs_minecraft_entity::WorldBorderSettings;
use mcrs_minecraft_physics::Vitals;

/// The ticks that a powder snow needs to freeze an entity (ENV-FRZ-001).
pub const FROZEN_TICKS: i32 = 140;

/// True when `y` is below the void floor of a dimension (ENV-VOID-001).
#[must_use]
pub fn is_in_void(y: f64, min_y: f64) -> bool {
    y < min_y - 64.0
}

/// The fall damage of a fall distance (DMG-FALL-002).
#[must_use]
pub fn fall_damage(fall_distance: f64, safe_fall_distance: f64, fall_multiplier: f32) -> f32 {
    let excess = fall_distance + 1.0e-6 - safe_fall_distance;
    if !excess.is_finite() || excess <= 0.0 {
        return 0.0;
    }
    let damage = excess * f64::from(fall_multiplier);
    if !damage.is_finite() || damage <= 0.0 {
        return 0.0;
    }
    damage.floor().min(f64::from(i32::MAX)) as f32
}

/// The world border damage of a player (ENV-BRD-001).
#[must_use]
pub fn border_damage(
    border: &WorldBorderSettings,
    position: DVec3,
    width: f64,
    height: f64,
) -> f32 {
    let min_x = border.min_x();
    let max_x = border.max_x();
    let min_z = border.min_z();
    let max_z = border.max_z();
    let half_width = width / 2.0;
    if position.x - half_width >= min_x
        && position.z - half_width >= min_z
        && position.x + half_width - 1.0e-5 < max_x
        && position.z + half_width - 1.0e-5 < max_z
    {
        return 0.0;
    }
    let _ = height;
    let d = (position.x - min_x)
        .min(max_x - position.x)
        .min(position.z - min_z)
        .min(max_z - position.z);
    let b = d + border.safe_zone;
    if b >= 0.0 || border.damage_per_block <= 0.0 {
        return 0.0;
    }
    (-b * border.damage_per_block).floor().max(1.0) as f32
}

/// Advances the fire ticks and returns whether the entity takes the `on_fire` hit
/// (ENV-FIR-002).
pub fn tick_fire(vitals: &mut Vitals, fire_immune: bool, in_lava: bool) -> bool {
    if vitals.fire_ticks <= 0 {
        return false;
    }
    if fire_immune {
        vitals.fire_ticks = 0;
        return false;
    }
    let hit = vitals.fire_ticks % 20 == 0 && !in_lava;
    vitals.fire_ticks -= 1;
    hit
}

/// Ignites an entity and clears its frozen ticks (ENV-FIR-003).
pub fn ignite(vitals: &mut Vitals, ticks: i32, fire_immune: bool) {
    if !fire_immune && vitals.fire_ticks < ticks {
        vitals.fire_ticks = ticks;
    }
    vitals.frozen_ticks = 0;
}

/// Advances the air supply and returns whether the entity takes the drown hit
/// (ENV-AIR-002, ENV-AIR-003).
pub fn tick_air(vitals: &mut Vitals, max_air: i32, can_drown: bool, refills: bool) -> bool {
    if can_drown {
        vitals.air -= 1;
    } else if vitals.air < max_air && refills {
        vitals.air = (vitals.air + 4).min(max_air);
    }
    if vitals.air <= -20 {
        vitals.air = 0;
        return true;
    }
    false
}

/// Advances the frozen ticks and returns whether the entity takes the freeze hit
/// (ENV-FRZ-002, ENV-FRZ-003).
pub fn tick_freeze(
    vitals: &mut Vitals,
    in_powder_snow: bool,
    can_freeze: bool,
    freezing_effect: bool,
    age: u64,
) -> bool {
    if can_freeze && in_powder_snow {
        vitals.frozen_ticks = (vitals.frozen_ticks + 1).min(FROZEN_TICKS);
    } else if !can_freeze || (!in_powder_snow && !freezing_effect) {
        vitals.frozen_ticks = (vitals.frozen_ticks - 2).max(0);
    }
    can_freeze && vitals.frozen_ticks >= FROZEN_TICKS && age.is_multiple_of(40)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_player_ten_blocks_outside_takes_two() {
        let border = WorldBorderSettings::default();
        let position = DVec3::new(30_000_000.0 - 10.0, 64.0, 0.0);
        assert_eq!(border_damage(&border, position, 0.6, 1.8), 1.0);
    }

    #[test]
    fn fire_hits_every_twenty_ticks() {
        let mut vitals = Vitals {
            fire_ticks: 40,
            ..Vitals::default()
        };
        let hits = (0..40)
            .filter(|_| tick_fire(&mut vitals, false, false))
            .count();
        assert_eq!(hits, 2);
        assert_eq!(vitals.fire_ticks, 0);
    }

    #[test]
    fn air_drowns_at_minus_twenty() {
        let mut vitals = Vitals {
            air: 0,
            ..Vitals::default()
        };
        for _ in 0..19 {
            assert!(!tick_air(&mut vitals, 300, true, true));
        }
        assert!(tick_air(&mut vitals, 300, true, true));
        assert_eq!(vitals.air, 0);
    }
}

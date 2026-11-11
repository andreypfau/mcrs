/// Reduces damage `x` by armor value `v` and toughness `t` (DMG-RED-001).
#[must_use]
pub fn reduce_by_armor(x: f32, v: f32, t: f32) -> f32 {
    if v <= 0.0 || x <= 0.0 {
        return x;
    }
    let k = 2.0 + t / 4.0;
    let r = (v - x / k).clamp(v * 0.2, 20.0);
    x * (1.0 - r / 25.0)
}

/// Reduces damage `x` by resistance amplifier `p` (DMG-RED-002).
#[must_use]
pub fn reduce_by_resistance(x: f32, p: u8) -> f32 {
    let n = 25 - (i32::from(p) + 1) * 5;
    if n <= 0 {
        0.0
    } else {
        ((x * (n as f32)) / 25.0).max(0.0)
    }
}

/// Computes the new health and absorption from applied damage `y` (DMG-HIT-004, DMG-RED-004).
#[must_use]
pub fn apply_absorption_and_health(
    health: f32,
    max_health: f32,
    absorption: f32,
    y: f32,
) -> (f32, f32) {
    let h = (y - absorption).max(0.0);
    let new_absorption = absorption - (y - h);
    let new_health = (health - h).clamp(0.0, max_health);
    (new_health, new_absorption)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn red_001_zombie_with_armor_2_on_magma() {
        // Spec DMG-RED-001: zombie with armor 2, toughness 0, taking 1.0 damage
        let damage = 1.0;
        let reduced = reduce_by_armor(damage, 2.0, 0.0);
        assert!((reduced - 0.94).abs() < 1e-6);
    }

    #[test]
    fn red_002_resistance_levels() {
        // Resistance 0 (amplifier 0) -> 20% reduction (80% remaining)
        assert!((reduce_by_resistance(10.0, 0) - 8.0).abs() < 1e-6);
        // Resistance 1 -> 40% reduction (60% remaining)
        assert!((reduce_by_resistance(10.0, 1) - 6.0).abs() < 1e-6);
        // Resistance 3 -> 80% reduction (20% remaining)
        assert!((reduce_by_resistance(10.0, 3) - 2.0).abs() < 1e-6);
        // Resistance 4 -> complete immunity
        assert_eq!(reduce_by_resistance(10.0, 4), 0.0);
        // Resistance 5 -> complete immunity
        assert_eq!(reduce_by_resistance(10.0, 5), 0.0);
    }

    #[test]
    fn red_004_absorption_absorbs_damage_first() {
        // 5 damage against 4 absorption and 20 health:
        // Absorption absorbs 4, leaves 0.
        // Remaining 1 damage reduces health to 19.
        let (new_health, new_absorption) = apply_absorption_and_health(20.0, 20.0, 4.0, 5.0);
        assert_eq!(new_absorption, 0.0);
        assert_eq!(new_health, 19.0);
    }

    #[test]
    fn red_004_absorption_exceeding_damage() {
        // 3 damage against 5 absorption and 20 health:
        // Absorption absorbs 3, leaves 2. Health unaffected.
        let (new_health, new_absorption) = apply_absorption_and_health(20.0, 20.0, 5.0, 3.0);
        assert_eq!(new_absorption, 2.0);
        assert_eq!(new_health, 20.0);
    }
}

use glam::DVec3;

/// Computes the knockback velocity impulse on an entity (DMG-HIT-005).
#[must_use]
pub fn compute_knockback(
    target_pos: DVec3,
    attacker_pos: DVec3,
    strength: f64,
    knockback_resistance: f64,
    current_velocity: DVec3,
) -> DVec3 {
    let factor = (1.0 - knockback_resistance).clamp(0.0, 1.0);
    if factor <= 0.0 || strength <= 0.0 {
        return current_velocity;
    }

    let mut dx = target_pos.x - attacker_pos.x;
    let mut dz = target_pos.z - attacker_pos.z;

    let dist_sq = dx * dx + dz * dz;
    if dist_sq < 1e-4 {
        dx = 0.1;
        dz = 0.0;
    } else {
        let dist = dist_sq.sqrt();
        dx /= dist;
        dz /= dist;
    }

    let impulse_h = strength * factor;
    let impulse_v = (strength * factor).min(0.4);

    let vx = current_velocity.x / 2.0 + dx * impulse_h;
    let vy = (current_velocity.y / 2.0 + impulse_v).min(0.4);
    let vz = current_velocity.z / 2.0 + dz * impulse_h;

    DVec3::new(vx, vy, vz)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_resistance_prevents_knockback() {
        let current = DVec3::new(0.0, 0.0, 0.0);
        let target = DVec3::new(10.0, 64.0, 0.0);
        let attacker = DVec3::new(9.0, 64.0, 0.0);
        let result = compute_knockback(target, attacker, 0.4, 1.0, current);
        assert_eq!(result, current);
    }

    #[test]
    fn knockback_pushes_away_from_attacker() {
        let current = DVec3::new(0.0, 0.0, 0.0);
        let target = DVec3::new(10.0, 64.0, 0.0);
        let attacker = DVec3::new(5.0, 64.0, 0.0);
        let result = compute_knockback(target, attacker, 0.4, 0.0, current);

        // Target is at X=10, attacker at X=5, so target should be pushed +X
        assert!(result.x > 0.0);
        assert_eq!(result.z, 0.0);
        assert!(result.y > 0.0);
    }
}

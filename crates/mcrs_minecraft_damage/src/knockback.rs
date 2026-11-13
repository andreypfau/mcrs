use glam::DVec3;

/// Computes the knockback velocity impulse on an entity (DMG-HIT-005).
#[must_use]
pub fn compute_knockback(
    target_pos: DVec3,
    attacker_pos: DVec3,
    strength: f64,
    knockback_resistance: f64,
    on_ground: bool,
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
        dx = 0.01;
        dz = 0.0;
    }
    let dist = (dx * dx + dz * dz).sqrt();
    dx /= dist;
    dz /= dist;

    let impulse_h = strength * factor;
    let vx = current_velocity.x / 2.0 + dx * impulse_h;
    let vy = if on_ground {
        (current_velocity.y / 2.0 + impulse_h).min(0.4)
    } else {
        current_velocity.y
    };
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
        let result = compute_knockback(target, attacker, 0.4, 1.0, true, current);
        assert_eq!(result, current);
    }

    #[test]
    fn knockback_pushes_away_from_attacker() {
        let current = DVec3::new(0.0, 0.0, 0.0);
        let target = DVec3::new(10.0, 64.0, 0.0);
        let attacker = DVec3::new(5.0, 64.0, 0.0);
        let result = compute_knockback(target, attacker, 0.4, 0.0, true, current);

        // Target is at X=10, attacker at X=5, so target should be pushed +X
        assert!(result.x > 0.0);
        assert_eq!(result.z, 0.0);
        assert!(result.y > 0.0);
    }

    #[test]
    fn airborne_knockback_keeps_the_vertical_velocity() {
        let current = DVec3::new(0.0, -0.3, 0.0);
        let target = DVec3::new(10.0, 64.0, 0.0);
        let attacker = DVec3::new(5.0, 64.0, 0.0);
        let result = compute_knockback(target, attacker, 0.4, 0.0, false, current);

        assert_eq!(result.y, current.y);
    }
}

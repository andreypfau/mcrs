use glam::DVec3;
use mcrs_minecraft_physics::Aabb;

const MARGIN: f64 = 3.0;

#[must_use]
pub fn in_attack_reach(eye: DVec3, target: Aabb, entity_range: f64) -> bool {
    let nearest = eye.max(target.min).min(target.max);
    eye.distance(nearest) <= f64::from(entity_range as f32) + MARGIN
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reach_at_the_bounds() {
        let pig = Aabb::new(DVec3::new(-0.45, 0.0, 0.0), DVec3::new(0.45, 0.9, 0.9));
        let cases = [
            (DVec3::new(0.0, 0.5, -5.95), 3.0, true),
            (DVec3::new(0.0, 0.5, -6.0), 3.0, true),
            (DVec3::new(0.0, 0.5, -6.05), 3.0, false),
            (DVec3::new(0.0, 0.5, 0.5), 3.0, true),
            (DVec3::new(0.0, 0.5, -9.95), 7.0, true),
            (DVec3::new(0.0, 0.5, -10.05), 7.0, false),
            (DVec3::new(0.0, 0.5, -6.0), 2.999_999_999, true),
        ];
        for (eye, range, expected) in cases {
            assert_eq!(
                in_attack_reach(eye, pig, range),
                expected,
                "eye {eye}, range {range}"
            );
        }
    }
}

//! Angles in degrees.

use mcrs_minecraft_numeric::wrap_degrees;

/// The angle a fraction `t` of the shorter arc from `from` to `to`.
#[must_use]
pub fn alerp(t: f32, from: f32, to: f32) -> f32 {
    from + t * wrap_degrees(to - from)
}

/// `angle`, moved by whole turns so that `reference - angle` is in `[-180, 180)`.
#[must_use]
pub fn unwrap_towards(angle: f32, reference: f32) -> f32 {
    reference - wrap_degrees(reference - angle)
}

/// `angle` when it lies within `limit` of `target`; otherwise the angle at the distance `limit`
/// from `target` on the side of `angle`.
#[must_use]
pub fn stay_within(angle: f32, target: f32, limit: f32) -> f32 {
    let offset = wrap_degrees(target - angle);
    if offset.abs() <= limit {
        angle
    } else {
        target - limit.copysign(offset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alerp_follows_the_shorter_arc() {
        assert_eq!(alerp(0.5, 170.0, -170.0), 180.0);
        assert_eq!(alerp(0.5, -170.0, 170.0), -180.0);
    }

    #[test]
    fn stay_within_stops_on_the_side_of_the_angle() {
        assert_eq!(stay_within(10.0, 0.0, 75.0), 10.0);
        assert_eq!(stay_within(100.0, 0.0, 75.0), 75.0);
        assert_eq!(stay_within(-100.0, 0.0, 75.0), -75.0);
        assert_eq!(stay_within(200.0, 0.0, 75.0), -75.0);
    }
}

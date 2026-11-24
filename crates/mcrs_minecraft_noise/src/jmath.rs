#[inline]
pub fn vmin(left: f32, right: f32) -> f32 {
    if right < left { right } else { left }
}

#[inline]
pub fn vmax(left: f32, right: f32) -> f32 {
    if right > left { right } else { left }
}

#[inline(always)]
pub fn mul_add(a: f32, b: f32, c: f32) -> f32 {
    #[cfg(feature = "fast")]
    return a.mul_add(b, c);
    #[cfg(not(feature = "fast"))]
    return a * b + c;
}

#[inline(always)]
pub fn mul_add64(a: f64, b: f64, c: f64) -> f64 {
    #[cfg(feature = "fast")]
    return a.mul_add(b, c);
    #[cfg(not(feature = "fast"))]
    return a * b + c;
}

#[inline(always)]
pub fn lerp(delta: f32, from: f32, to: f32) -> f32 {
    mul_add(delta, to - from, from)
}

#[inline]
pub fn sampler_lerp(alpha: f32, first: f32, second: f32) -> f32 {
    if alpha == 0.0 {
        first
    } else if alpha == 1.0 {
        second
    } else {
        lerp(alpha, first, second)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn min_max_follow_the_volume_path_not_math_min() {
        assert_eq!(vmin(3.0, 5.0), 3.0);
        assert_eq!(vmax(3.0, 5.0), 5.0);
        assert!(vmin(0.0, -0.0).is_sign_positive());
        assert!(vmax(-0.0, 0.0).is_sign_negative());
    }

    #[test]
    fn a_unit_alpha_lerps_to_the_endpoint_exactly() {
        let (first, second) = (-0.524_070_74_f32, 0.088_458_45_f32);
        assert_eq!(sampler_lerp(1.0, first, second), second);
        assert_eq!(sampler_lerp(0.0, first, second), first);
        assert_ne!(
            lerp(1.0, first, second),
            second,
            "the unbranched lerp misses the endpoint, which is why the samplers branch"
        );
    }
}

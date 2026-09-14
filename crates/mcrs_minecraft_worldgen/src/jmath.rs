//! Java float semantics. Rust's stdlib disagrees with `java.lang.Math` on signed
//! zero and on division rounding often enough that using the stdlib directly is
//! a silent parity bug.
//!
//! Density values are guaranteed NaN-free, so nothing here propagates NaN. If
//! that guarantee is ever broken these functions return the wrong operand
//! silently; `Program::fill` carries the debug assertion that catches it.
//!
//! What lives here rather than in `mcrs_voxel_math::mth` is what the `fast`
//! profile changes: moving it down would switch the profile on for every crate
//! the build unifies with.

/// `min` and `max` on density values, as the *volume* path computes them: a bare
/// comparison with the left operand as the accumulator. Vanilla's scalar path
/// uses `Math.min` / `Math.max` instead, and the two disagree on signed zero —
/// the parity fixtures are volume dumps, so the volume form is the one to match.
#[inline]
pub fn vmin(left: f32, right: f32) -> f32 {
    if right < left { right } else { left }
}

#[inline]
pub fn vmax(left: f32, right: f32) -> f32 {
    if right > left { right } else { left }
}

/// `a * b + c`. The fast profile fuses the multiply and the add into one
/// rounding, which is the trade `docs/worldgen.md` §15 lists for that profile;
/// the strict profile keeps the two roundings Java computes.
///
/// On a target without an FMA instruction this lowers to a libm call and the
/// fast profile is *slower* than the strict one, so build it with the feature
/// enabled (`target-cpu=native`, or `+fma` on x86-64).
#[inline(always)]
pub fn mul_add(a: f32, b: f32, c: f32) -> f32 {
    #[cfg(feature = "fast")]
    return a.mul_add(b, c);
    #[cfg(not(feature = "fast"))]
    return a * b + c;
}

/// [`mul_add`] over the width the lattice coordinates keep.
#[inline(always)]
pub fn mul_add64(a: f64, b: f64, c: f64) -> f64 {
    #[cfg(feature = "fast")]
    return a.mul_add(b, c);
    #[cfg(not(feature = "fast"))]
    return a * b + c;
}

/// `Mth.lerp(float delta, float from, float to)`. Always inlined: the noise
/// kernels call it eight times per lattice cell.
#[inline(always)]
pub fn lerp(delta: f32, from: f32, to: f32) -> f32 {
    mul_add(delta, to - from, from)
}

/// The lerp every `LerpFunction` sampler computes, which is not `Mth.lerp`: an
/// alpha of exactly zero or one returns that endpoint untouched, where
/// `first + 1 * (second - first)` misses `second` by an ulp.
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
        // Math.min(+0.0, -0.0) is -0.0; the volume path keeps the accumulator.
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
            "the bare Mth.lerp misses the endpoint, which is why the samplers branch"
        );
    }
}

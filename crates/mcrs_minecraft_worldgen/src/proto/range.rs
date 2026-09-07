use crate::interval::Interval;
use crate::jmath::{clampf, jmax, jmin};
use crate::program::RoundKind;
use crate::proto::{
    DensityFunctionHolder, NoiseHolder, NoiseParam, ProtoDensityFunction, ProtoMultipoint,
    ProtoSpline,
};
use mcrs_minecraft_core::ResourceLocation;
use std::collections::BTreeMap;

/// Runs after reference inlining and after cache preparation, so there is
/// neither a `Reference` arm nor a `Cache` arm.
///
/// The bound vanilla *declares*, which branch elimination reads. For a noise it
/// is a six-sigma statistical bound on the summed octaves, so a sample may
/// legitimately fall outside it.
///
/// That is why [`crate::cell::CellBounds`] cannot reuse this and computes its
/// own: a cell bound decides substance without sampling, so it has to be
/// rigorous, and a statistical bound would write stone through air.
///
/// Unlike `domain_axes`, this needs the noise registry: `noise`, `shift`,
/// `shift_a` and `shift_b` all report the amplitude bound of a noise that may
/// be named rather than inlined.
pub fn range(
    noises: &BTreeMap<ResourceLocation, NoiseParam>,
    f: &ProtoDensityFunction,
) -> Interval {
    use ProtoDensityFunction::*;
    let r = |h: &DensityFunctionHolder| holder_range(noises, h);
    match f {
        Constant(c) => Interval::exact(c.value.0 as f32),
        // Conservative on purpose: with no blender the sampler returns only the
        // fallback, but the declared bound is what branch elimination reads and
        // narrowing it would delete branches vanilla keeps.
        BlendAlpha => Interval::of(0.0, 1.0),
        BlendOffset | Beardifier => Interval::INFINITE,
        // The shifts are not consulted and do not widen it.
        Noise { noise, .. } => noise_range(noises, noise),
        Shift { noise } | ShiftA { noise } | ShiftB { noise } => {
            noise_range(noises, noise) * Interval::exact(4.0)
        }
        EndOuterIslands => Interval::of(-0.84375, 0.5625),
        DistanceToPoint { .. } => Interval::of(0.0, f32::INFINITY),
        Gradient(x) => Interval::encapsulating(x.from_value.0 as f32, x.to_value.0 as f32),
        OldBlendedNoise(x) => {
            crate::node::blended::declared_range(x.y_scale.0, x.smear_scale_multiplier.0)
        }
        Abs(x) => r(&x.input).abs(),
        Square(x) => r(&x.input).square(),
        Cube(x) => r(&x.input).map_monotonic(|v| (v * v) * v),
        // Not `pointwise_max(0)` then map: for an entirely negative input
        // vanilla yields NaI where the clamp-then-map form yields [0, 0].
        Sqrt(x) => r(&x.input).pow(Interval::exact(0.5)),
        HalfNegative(x) => r(&x.input).map_monotonic(|v| if v > 0.0 { v } else { v * 0.5 }),
        QuarterNegative(x) => r(&x.input).map_monotonic(|v| if v > 0.0 { v } else { v * 0.25 }),
        Reciprocal(x) => r(&x.input).reciprocal(),
        Negate(x) => Interval::exact(0.0) - r(&x.input),
        Squeeze(x) => r(&x.input).map_monotonic(|v| {
            let c = clampf(v, -1.0, 1.0);
            c / 2.0 - (c * c * c) / 24.0
        }),
        Log(x) => r(&x.input).log(),
        Sign(x) => r(&x.input).sign(),
        Clamp(x) => r(&x.input).clamped(x.min.0 as f32, x.max.0 as f32),
        Floor(x) => round_range(r(&x.input), r(&x.multiple), RoundKind::Floor),
        Round(x) => round_range(r(&x.input), r(&x.multiple), RoundKind::Round),
        Ceil(x) => round_range(r(&x.input), r(&x.multiple), RoundKind::Ceil),
        Truncate(x) => round_range(r(&x.input), r(&x.multiple), RoundKind::Truncate),
        Add(x) => r(&x.left) + r(&x.right),
        Sub(x) => r(&x.left) - r(&x.right),
        Mul(x) => r(&x.left) * r(&x.right),
        Div(x) => r(&x.left) / r(&x.right),
        Min(x) => r(&x.left).pointwise_min(r(&x.right)),
        Max(x) => r(&x.left).pointwise_max(r(&x.right)),
        Pow(x) => r(&x.base).pow(r(&x.exponent)),
        Lerp {
            alpha,
            first,
            second,
        } => Interval::lerp(r(alpha), r(first), r(second)),
        Spline { spline } => spline_range(noises, spline),
        // The input's range is never consulted, so no branch is eliminated.
        RangeChoice {
            when_in_range,
            when_out_of_range,
            ..
        } => r(when_in_range).union(r(when_out_of_range)),
        // Over the branches only: the mirror image of `domain_axes`, which
        // includes the input.
        IntervalSelect(x) => x
            .functions
            .iter()
            .fold(Interval::NAI, |acc, f| acc.union(r(f))),
        BlendDensity(x) => r(&x.input),
        Interpolated { input, .. } => r(input),
        Slice { input, .. } => r(input),
        // `jmax`, so a NaI upper bound propagates NaN into `Interval::of` and
        // aborts the load, as vanilla does.
        FindTopSurface(x) => {
            let lower = x.lower_bound as f32;
            Interval::of(lower, jmax(lower, r(&x.upper_bound).max()))
        }
        Cache(_) => unreachable!("cache survived preparation"),
    }
}

pub fn holder_range(
    noises: &BTreeMap<ResourceLocation, NoiseParam>,
    holder: &DensityFunctionHolder,
) -> Interval {
    match holder {
        DensityFunctionHolder::Value(c) => Interval::exact(c.value.0 as f32),
        DensityFunctionHolder::Owned(f) => range(noises, f),
        DensityFunctionHolder::Reference(id) => unreachable!("reference {id} survived inlining"),
    }
}

/// Does not model the sampler's `multiple == 0.0` passthrough: an exactly
/// `[0, 0]` multiple yields NaI here while the sampler returns its input.
/// Vanilla has the same omission.
pub fn round_range(value: Interval, multiple: Interval, kind: RoundKind) -> Interval {
    (value / multiple).map_monotonic(|v| kind.apply(v)) * multiple
}

fn noise_range(noises: &BTreeMap<ResourceLocation, NoiseParam>, holder: &NoiseHolder) -> Interval {
    match holder {
        NoiseHolder::Owned(params) => params.range(),
        NoiseHolder::Reference(id) => noises
            .get(id)
            .unwrap_or_else(|| panic!("unknown noise {id}"))
            .range(),
    }
}

fn spline_range(noises: &BTreeMap<ResourceLocation, NoiseParam>, spline: &ProtoSpline) -> Interval {
    match spline {
        ProtoSpline::Constant(v) => Interval::exact(*v),
        ProtoSpline::Multipoint(m) => multipoint_range(noises, m),
    }
}

fn multipoint_range(
    noises: &BTreeMap<ResourceLocation, NoiseParam>,
    s: &ProtoMultipoint,
) -> Interval {
    let input = holder_range(noises, &s.coordinate);
    if input.is_nai() {
        return input;
    }

    let last = s.locations.len() - 1;
    // Returns the value unchanged at a zero derivative rather than computing
    // `value + 0.0 * (x - loc)`, which differs when `x - loc` is infinite.
    let extend = |x: f32, value: f32, i: usize| {
        let d = s.derivatives[i];
        if d == 0.0 {
            value
        } else {
            value + d * (x - s.locations[i])
        }
    };

    let values: Vec<Interval> = s.values.iter().map(|v| spline_range(noises, v)).collect();

    let mut min_value = f32::INFINITY;
    let mut max_value = f32::NEG_INFINITY;

    if input.min() < s.locations[0] {
        let lo = extend(input.min(), values[0].min(), 0);
        let hi = extend(input.min(), values[0].max(), 0);
        min_value = jmin(min_value, jmin(lo, hi));
        max_value = jmax(max_value, jmax(lo, hi));
    }
    if input.max() > s.locations[last] {
        let lo = extend(input.max(), values[last].min(), last);
        let hi = extend(input.max(), values[last].max(), last);
        min_value = jmin(min_value, jmin(lo, hi));
        max_value = jmax(max_value, jmax(lo, hi));
    }

    for v in &values {
        min_value = jmin(min_value, v.min());
        max_value = jmax(max_value, v.max());
    }

    for i in 0..last {
        let d1 = s.derivatives[i];
        let d2 = s.derivatives[i + 1];
        // Skipping this guard widens the interval on every flat segment of the
        // shipped continentalness and erosion splines.
        if d1 == 0.0 && d2 == 0.0 {
            continue;
        }
        let x_diff = s.locations[i + 1] - s.locations[i];
        let (r1, r2) = (values[i], values[i + 1]);
        let p1 = d1 * x_diff;
        let p2 = d2 * x_diff;
        let min_lerp1 = jmin(r1.min(), r2.min());
        let max_lerp1 = jmax(r1.max(), r2.max());
        let min_lerp2 = jmin(p1 - r2.max() + r1.min(), -p2 + r2.min() - r1.max());
        let max_lerp2 = jmax(p1 - r2.min() + r1.max(), -p2 + r2.max() - r1.min());
        min_value = jmin(min_value, min_lerp1 + 0.25 * min_lerp2);
        max_value = jmax(max_value, max_lerp1 + 0.25 * max_lerp2);
    }

    Interval::of(min_value, max_value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_noises() -> BTreeMap<ResourceLocation, NoiseParam> {
        BTreeMap::new()
    }

    fn range_of(json: &str) -> Interval {
        let f: ProtoDensityFunction = serde_json::from_str(json).unwrap();
        range(&no_noises(), &f)
    }

    /// The input's range would confine this to the out-of-range branch, and
    /// vanilla still reports the hull of both.
    #[test]
    fn range_choice_keeps_the_unreachable_branch() {
        let bounds = range_of(
            r#"{"type":"minecraft:range_choice","input":100.0,"min_inclusive":0.0,"max_exclusive":1.0,
                "when_in_range":-5.0,"when_out_of_range":5.0}"#,
        );
        assert_eq!((bounds.min(), bounds.max()), (-5.0, 5.0));
    }

    /// `interval_select` is the mirror image: the input is excluded here and
    /// included by `domain_axes`.
    #[test]
    fn interval_select_ignores_its_input() {
        let bounds = range_of(
            r#"{"type":"minecraft:interval_select","input":-1000.0,"thresholds":[0.0],"functions":[1.0,2.0]}"#,
        );
        assert_eq!((bounds.min(), bounds.max()), (1.0, 2.0));
    }

    /// A flat segment must not pick up the ±0.25 overshoot term, and the end
    /// probes are strict, so a coordinate inside the span adds nothing either.
    #[test]
    fn a_flat_spline_is_exactly_its_values() {
        let bounds = range_of(
            r#"{"type":"minecraft:spline","spline":{"coordinate":0.5,"points":[
                {"location":0.0,"value":1.0,"derivative":0.0},
                {"location":1.0,"value":3.0,"derivative":0.0}]}}"#,
        );
        assert_eq!((bounds.min(), bounds.max()), (1.0, 3.0));

        let sloped = range_of(
            r#"{"type":"minecraft:spline","spline":{"coordinate":0.5,"points":[
                {"location":0.0,"value":1.0,"derivative":1.0},
                {"location":1.0,"value":3.0,"derivative":0.0}]}}"#,
        );
        assert!(sloped.max() > 3.0, "the overshoot term widens it");
    }

    #[test]
    fn a_wholly_negative_sqrt_is_unknown_rather_than_zero() {
        assert!(range_of(r#"{"type":"minecraft:sqrt","input":-4.0}"#).is_nai());
    }

    /// The clamp-alpha lerp cannot leave the two limit noises' common interval,
    /// so the declared bound is one fbm's.
    #[test]
    fn the_overworld_blended_noise_matches_its_published_bound() {
        let bounds = range_of(
            r#"{"type":"minecraft:old_blended_noise","xz_scale":0.25,"y_scale":0.125,
                "xz_factor":80.0,"y_factor":160.0,"smear_scale_multiplier":8.0}"#,
        );
        assert_eq!(bounds.max(), 2.1670623);
        assert_eq!(bounds.min(), -2.1670623);
    }
}

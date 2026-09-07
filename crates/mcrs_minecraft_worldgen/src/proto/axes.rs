use crate::proto::{DensityFunctionHolder, ProtoDensityFunction, ProtoSpline};
use crate::volume::Axis;

pub use crate::strata::{ALL_AXES, AXIS_X, AXIS_Y, AXIS_Z, NO_AXES};

#[inline]
pub fn axes_from(axis: Axis) -> u8 {
    match axis {
        Axis::X => AXIS_X,
        Axis::Y => AXIS_Y,
        Axis::Z => AXIS_Z,
    }
}

/// Runs after reference inlining and after cache preparation, so there is
/// neither a `Reference` arm nor a `Cache` arm.
pub fn domain_axes(f: &ProtoDensityFunction) -> u8 {
    use ProtoDensityFunction::*;
    match f {
        Constant(_) => NO_AXES,
        // `blend_alpha` and `blend_offset` declare X|Z regardless of what an
        // injected blender would do, and `blend_density` reports its input's
        // axes though a real blender varies with position. Both unsoundnesses
        // are vanilla's; correcting either moves the inserted slices.
        BlendAlpha | BlendOffset => AXIS_X | AXIS_Z,
        Beardifier => ALL_AXES,
        Noise {
            xz_scale,
            y_scale,
            shift_x,
            shift_y,
            shift_z,
            ..
        } => {
            let mut axes = ALL_AXES;
            if y_scale.0 == 0.0 {
                axes &= !AXIS_Y;
            }
            if xz_scale.0 == 0.0 {
                axes &= !(AXIS_X | AXIS_Z);
            }
            axes | holder_axes(shift_x) | holder_axes(shift_y) | holder_axes(shift_z)
        }
        Shift { .. } => ALL_AXES,
        ShiftA { .. } | ShiftB { .. } | EndOuterIslands => AXIS_X | AXIS_Z,
        DistanceToPoint { .. } => ALL_AXES,
        Gradient(x) => axes_from(x.axis),
        OldBlendedNoise(_) => ALL_AXES,
        Abs(x) | Square(x) | Cube(x) | Sqrt(x) | HalfNegative(x) | QuarterNegative(x)
        | Reciprocal(x) | Negate(x) | Squeeze(x) | Log(x) | Sign(x) | BlendDensity(x) => {
            holder_axes(&x.input)
        }
        Clamp(x) => holder_axes(&x.input),
        Floor(x) | Round(x) | Ceil(x) | Truncate(x) => {
            holder_axes(&x.input) | holder_axes(&x.multiple)
        }
        Add(x) | Sub(x) | Mul(x) | Div(x) | Min(x) | Max(x) => {
            holder_axes(&x.left) | holder_axes(&x.right)
        }
        Pow(x) => holder_axes(&x.base) | holder_axes(&x.exponent),
        Lerp {
            alpha,
            first,
            second,
        } => holder_axes(alpha) | holder_axes(first) | holder_axes(second),
        Spline { spline } => spline_axes(spline),
        RangeChoice {
            input,
            when_in_range,
            when_out_of_range,
            ..
        } => holder_axes(input) | holder_axes(when_in_range) | holder_axes(when_out_of_range),
        IntervalSelect(x) => x
            .functions
            .iter()
            .fold(holder_axes(&x.input), |axes, f| axes | holder_axes(f)),
        Interpolated { input, .. } => holder_axes(input),
        Slice { axis, input, .. } => holder_axes(input) & !axes_from(*axis),
        FindTopSurface(x) => (holder_axes(&x.density) | holder_axes(&x.upper_bound)) & !AXIS_Y,
        Cache(_) => unreachable!("cache survived preparation"),
    }
}

pub fn holder_axes(holder: &DensityFunctionHolder) -> u8 {
    match holder {
        DensityFunctionHolder::Value(_) => NO_AXES,
        DensityFunctionHolder::Owned(f) => domain_axes(f),
        DensityFunctionHolder::Reference(id) => unreachable!("reference {id} survived inlining"),
    }
}

pub fn spline_axes(spline: &ProtoSpline) -> u8 {
    let mut axes = NO_AXES;
    spline.visit_coordinates(&mut |coordinate| axes |= holder_axes(coordinate));
    axes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(json: &str) -> ProtoDensityFunction {
        serde_json::from_str(json).unwrap()
    }

    /// A zero scale is the only way a `noise` sheds an axis, and the shifts add
    /// theirs back on top of whatever survives.
    #[test]
    fn a_zero_scale_drops_the_axes_it_flattens() {
        let flat_y = parse(
            r#"{"type":"minecraft:noise","noise":"minecraft:ridge","xz_scale":1.0,"y_scale":0.0}"#,
        );
        assert_eq!(domain_axes(&flat_y), AXIS_X | AXIS_Z);

        let flat_xz = parse(
            r#"{"type":"minecraft:noise","noise":"minecraft:ridge","xz_scale":0.0,"y_scale":0.0}"#,
        );
        assert_eq!(domain_axes(&flat_xz), NO_AXES);

        let shifted = parse(
            r#"{"type":"minecraft:noise","noise":"minecraft:ridge","xz_scale":0.0,"y_scale":0.0,
                "shift_x":{"type":"minecraft:gradient","axis":"y","from_coordinate":0,"to_coordinate":1,"from_value":0.0,"to_value":1.0}}"#,
        );
        assert_eq!(domain_axes(&shifted), AXIS_Y);
    }

    #[test]
    fn slice_and_find_top_surface_subtract_their_axis() {
        let sliced = parse(
            r#"{"type":"minecraft:slice","axis":"x","coordinate":0,
                "input":{"type":"minecraft:noise","noise":"minecraft:ridge","xz_scale":1.0,"y_scale":1.0}}"#,
        );
        assert_eq!(domain_axes(&sliced), AXIS_Y | AXIS_Z);

        let surface = parse(
            r#"{"type":"minecraft:find_top_surface","lower_bound":-64,"cell_height":8,
                "density":{"type":"minecraft:noise","noise":"minecraft:ridge","xz_scale":1.0,"y_scale":1.0},
                "upper_bound":0.0}"#,
        );
        assert_eq!(domain_axes(&surface), AXIS_X | AXIS_Z);
    }
}

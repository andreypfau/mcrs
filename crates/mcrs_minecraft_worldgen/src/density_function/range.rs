use super::{
    Affine, BinaryOperation, DensityFunctionComponent, DependentDensityFunction,
    IndependentDensityFunction, LinearOperation, NoiseRouter, RoundingMode, UnaryOperation,
    round_to_integer,
};

pub(super) fn mul_range(min1: f32, max1: f32, min2: f32, max2: f32) -> (f32, f32) {
    let min = if min1 > 0.0 && min2 > 0.0 {
        min1 * min2
    } else if max1 < 0.0 && max2 < 0.0 {
        max1 * max2
    } else {
        (min1 * max2).min(max1 * min2)
    };

    let max = if min1 > 0.0 && min2 > 0.0 {
        max1 * max2
    } else if max1 < 0.0 && max2 < 0.0 {
        min1 * min2
    } else {
        (min1 * min2).max(max1 * max2)
    };

    (min, max)
}

// A negative base with a non-constant exponent falls back to the trivial
// interval rather than analysing sign alternation. Ranges are only ever used to
// prove work redundant, so a wider interval costs speed, never parity.
#[inline]
pub(super) fn pow_narrowed(base: f32, exponent: f32) -> f32 {
    (base as f64).powf(exponent as f64) as f32
}

pub(super) fn pow_range(base: (f32, f32), exponent: (f32, f32)) -> (f32, f32) {
    let is_constant = base.0 == base.1 && exponent.0 == exponent.1;
    if base.0 < 0.0 && !is_constant {
        return (f32::NEG_INFINITY, f32::INFINITY);
    }
    let corners = [
        pow_narrowed(base.0, exponent.0),
        pow_narrowed(base.0, exponent.1),
        pow_narrowed(base.1, exponent.0),
        pow_narrowed(base.1, exponent.1),
    ];
    if corners.iter().any(|v| v.is_nan()) {
        return (f32::NEG_INFINITY, f32::INFINITY);
    }
    corners
        .iter()
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(min, max), &value| {
            (min.min(value), max.max(value))
        })
}

pub(super) fn binary_range(
    operation: BinaryOperation,
    (min1, max1): (f32, f32),
    (min2, max2): (f32, f32),
) -> (f32, f32) {
    match operation {
        BinaryOperation::Add => (min1 + min2, max1 + max2),
        BinaryOperation::Multiply => mul_range(min1, max1, min2, max2),
        BinaryOperation::Subtract => (min1 - max2, max1 - min2),
        BinaryOperation::Divide => {
            let (rmin, rmax) = reciprocal_range(min2, max2);
            mul_range(min1, max1, rmin, rmax)
        }
        BinaryOperation::Min => (min1.min(min2), max1.min(max2)),
        BinaryOperation::Max => (min1.max(min2), max1.max(max2)),
        BinaryOperation::Pow => pow_range((min1, max1), (min2, max2)),
        BinaryOperation::Round(mode) => {
            let (rmin, rmax) = reciprocal_range(min2, max2);
            let (dmin, dmax) = mul_range(min1, max1, rmin, rmax);
            let lo = round_to_integer(dmin, mode);
            let hi = round_to_integer(dmax, mode);
            mul_range(lo.min(hi), lo.max(hi), min2, max2)
        }
    }
}

pub(super) fn unary_range(operation: UnaryOperation, min: f32, max: f32) -> (f32, f32) {
    let min_image = operation.apply(min);
    let max_image = operation.apply(max);
    match operation {
        UnaryOperation::Reciprocal => reciprocal_range(min, max),
        UnaryOperation::Abs | UnaryOperation::Square => {
            if min >= 0.0 {
                (min_image, max_image)
            } else if max <= 0.0 {
                (max_image, min_image)
            } else {
                (0.0, min_image.max(max_image))
            }
        }
        UnaryOperation::Sqrt | UnaryOperation::Log => {
            (operation.apply(min.max(0.0)), operation.apply(max.max(0.0)))
        }
        UnaryOperation::Sign => {
            if min > 0.0 {
                (1.0, 1.0)
            } else if max < 0.0 {
                (-1.0, -1.0)
            } else {
                (
                    if min == 0.0 { 0.0 } else { -1.0 },
                    if max == 0.0 { 0.0 } else { 1.0 },
                )
            }
        }
        _ => (min_image, max_image),
    }
}

pub(super) fn reciprocal_range(min: f32, max: f32) -> (f32, f32) {
    if min == 0.0 && max == 0.0 {
        (f32::NEG_INFINITY, f32::INFINITY)
    } else if min > 0.0 || max < 0.0 {
        (1.0 / max, 1.0 / min)
    } else if max == 0.0 {
        (f32::NEG_INFINITY, 1.0 / min)
    } else if min == 0.0 {
        (1.0 / max, f32::INFINITY)
    } else {
        (f32::NEG_INFINITY, f32::INFINITY)
    }
}

impl NoiseRouter {
    /// Bounds on `final_density` across a whole cell, given each `interpolated`
    /// wrapper's own bounds over the cell's eight corners. Trilinear
    /// interpolation is a convex combination, so it never leaves the corner
    /// hull; interval arithmetic over the outer terms carries that up.
    ///
    /// `None` when an outer term has a kind this cannot bound, which simply
    /// means the caller must evaluate the cell block by block.
    pub fn final_density_cell_bounds(
        &self,
        wrapper_bounds: &[(f32, f32)],
        iv: &mut [(f32, f32)],
    ) -> Option<(f32, f32)> {
        for (k, &idx) in self.outer_wrappers.iter().enumerate() {
            iv[idx] = wrapper_bounds[k];
        }
        for &i in self.outer_terms.iter() {
            iv[i] = match &self.stack[i] {
                DensityFunctionComponent::Independent(IndependentDensityFunction::Constant(v)) => {
                    (*v, *v)
                }
                DensityFunctionComponent::Dependent(f) => match f {
                    DependentDensityFunction::Linear(x) => {
                        let (lo, hi) = iv[x.input_index];
                        match x.operation {
                            LinearOperation::Add => (lo + x.argument, hi + x.argument),
                            LinearOperation::Multiply => mul_range(lo, hi, x.argument, x.argument),
                        }
                    }
                    DependentDensityFunction::Affine(x) => {
                        let (lo, hi) = iv[x.input_index];
                        Affine::compute_range(lo, hi, x.scale, x.offset)
                    }
                    DependentDensityFunction::Unary(x) => {
                        let (lo, hi) = iv[x.input_index];
                        unary_range(x.operation, lo, hi)
                    }
                    DependentDensityFunction::Binary(x) => {
                        binary_range(x.operation, iv[x.input1_index], iv[x.input2_index])
                    }
                    DependentDensityFunction::Clamp(x) => {
                        let (lo, hi) = iv[x.input_index];
                        (
                            lo.clamp(x.min_value, x.max_value),
                            hi.clamp(x.min_value, x.max_value),
                        )
                    }
                    DependentDensityFunction::RangeChoice(x) => {
                        let (lo, hi) = iv[x.input_index];
                        if lo >= x.min_inclusion_value && hi < x.max_exclusion_value {
                            iv[x.when_in_index]
                        } else if hi < x.min_inclusion_value || lo >= x.max_exclusion_value {
                            iv[x.when_out_index]
                        } else {
                            let a = iv[x.when_in_index];
                            let b = iv[x.when_out_index];
                            (a.0.min(b.0), a.1.max(b.1))
                        }
                    }
                    _ => return None,
                },
                _ => return None,
            };
        }
        Some(iv[self.final_density_index])
    }
}

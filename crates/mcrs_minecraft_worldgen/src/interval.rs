use crate::jmath;

/// The bounds a value can take. Only ever widened: a bound narrower than the
/// truth would let the optimiser delete a reachable branch.
#[derive(Clone, Copy, Debug)]
pub struct Interval {
    min: f32,
    max: f32,
}

/// NaI compares equal to itself so that structurally identical stack entries
/// still deduplicate.
impl PartialEq for Interval {
    fn eq(&self, other: &Self) -> bool {
        (self.min == other.min && self.max == other.max) || (self.is_nai() && other.is_nai())
    }
}

impl Interval {
    /// "Not an Interval": nothing is known. Every ordering test against NaN is
    /// false, so a node carrying it survives every elimination the optimiser makes.
    pub const NAI: Self = Self {
        min: f32::NAN,
        max: f32::NAN,
    };

    pub const INFINITE: Self = Self {
        min: f32::NEG_INFINITY,
        max: f32::INFINITY,
    };

    /// The reference throws on an inverted or NaN pair. Nothing downstream can
    /// act on an inverted interval soundly, so it degrades to NaI instead.
    pub fn of(min: f32, max: f32) -> Self {
        if min.is_nan() || max.is_nan() || max < min {
            debug_assert!(false, "invalid interval [{min}; {max}]");
            return Self::NAI;
        }
        Self { min, max }
    }

    pub fn symmetric(range: f32) -> Self {
        Self::of(-range, range)
    }

    pub fn exact(value: f32) -> Self {
        Self::of(value, value)
    }

    #[inline]
    pub fn min(self) -> f32 {
        self.min
    }

    #[inline]
    pub fn max(self) -> f32 {
        self.max
    }

    #[inline]
    pub fn is_nai(self) -> bool {
        self.min.is_nan()
    }

    #[inline]
    pub fn contains(self, value: f32) -> bool {
        value >= self.min && value <= self.max
    }

    pub fn union(self, other: Self) -> Self {
        if self.is_nai() {
            return other;
        }
        if other.is_nai() {
            return self;
        }
        Self::of(self.min.min(other.min), self.max.max(other.max))
    }

    pub fn union_value(self, value: f32) -> Self {
        if value.is_nan() {
            return self;
        }
        if self.is_nai() {
            return Self::exact(value);
        }
        Self::of(self.min.min(value), self.max.max(value))
    }

    pub fn encapsulating(first: f32, second: f32) -> Self {
        match (first.is_nan(), second.is_nan()) {
            (true, true) => Self::NAI,
            (true, false) => Self::exact(second),
            (false, true) => Self::exact(first),
            (false, false) => Self::of(first.min(second), first.max(second)),
        }
    }

    fn or_nai(min: f32, max: f32) -> Self {
        if min.is_nan() || max.is_nan() {
            Self::NAI
        } else {
            Self::of(min, max)
        }
    }

    /// Zero times an infinite bound is zero here, not NaN: an endpoint pinned to
    /// zero contributes nothing however far the other side reaches.
    fn mul_bound(left: f32, right: f32) -> f32 {
        if left != 0.0 && right != 0.0 {
            left * right
        } else {
            0.0
        }
    }

    fn multiplied(self, other: Self) -> Self {
        if self.is_nai() || other.is_nai() {
            return Self::NAI;
        }
        let corners = [
            Self::mul_bound(self.min, other.min),
            Self::mul_bound(self.min, other.max),
            Self::mul_bound(self.max, other.min),
            Self::mul_bound(self.max, other.max),
        ];
        Self::of(
            corners.iter().copied().fold(f32::INFINITY, f32::min),
            corners.iter().copied().fold(f32::NEG_INFINITY, f32::max),
        )
    }

    pub fn reciprocal(self) -> Self {
        if self.is_nai() || (self.min == 0.0 && self.max == 0.0) {
            return Self::NAI;
        }
        if !self.contains(0.0) {
            Self::of(1.0 / self.max, 1.0 / self.min)
        } else if self.max == 0.0 {
            Self::of(f32::NEG_INFINITY, 1.0 / self.min)
        } else if self.min == 0.0 {
            Self::of(1.0 / self.max, f32::INFINITY)
        } else {
            Self::INFINITE
        }
    }

    pub fn pointwise_min(self, other: Self) -> Self {
        if self.is_nai() || other.is_nai() {
            return Self::NAI;
        }
        Self::of(self.min.min(other.min), self.max.min(other.max))
    }

    pub fn pointwise_max(self, other: Self) -> Self {
        if self.is_nai() || other.is_nai() {
            return Self::NAI;
        }
        Self::of(self.min.max(other.min), self.max.max(other.max))
    }

    pub fn clamped(self, min: f32, max: f32) -> Self {
        debug_assert!(min <= max, "clamp bounds inverted: [{min}; {max}]");
        if self.is_nai() {
            Self::NAI
        } else if self.min >= max {
            Self::exact(max)
        } else if self.max <= min {
            Self::exact(min)
        } else {
            Self::of(self.min.max(min), self.max.min(max))
        }
    }

    pub fn abs(self) -> Self {
        if self.is_nai() {
            return Self::NAI;
        }
        let max = self.min.abs().max(self.max.abs());
        if self.contains(0.0) {
            Self::of(0.0, max)
        } else {
            Self::of(self.min.abs().min(self.max.abs()), max)
        }
    }

    pub fn square(self) -> Self {
        if self.is_nai() {
            return Self::NAI;
        }
        let max = (self.min * self.min).max(self.max * self.max);
        if self.contains(0.0) {
            Self::of(0.0, max)
        } else {
            Self::of((self.min * self.min).min(self.max * self.max), max)
        }
    }

    pub fn sign(self) -> Self {
        if self.is_nai() {
            Self::NAI
        } else if self.min == self.max {
            Self::exact(jmath::signum(self.min))
        } else if !self.contains(0.0) {
            Self::exact(if self.min > 0.0 { 1.0 } else { -1.0 })
        } else if self.min == 0.0 {
            Self::of(0.0, 1.0)
        } else if self.max == 0.0 {
            Self::of(-1.0, 0.0)
        } else {
            Self::of(-1.0, 1.0)
        }
    }

    pub fn log(self) -> Self {
        if self.max < 0.0 {
            return Self::NAI;
        }
        self.pointwise_max(Self::exact(0.0))
            .map_monotonic(jmath::log)
    }

    pub fn map_monotonic(self, op: impl Fn(f32) -> f32) -> Self {
        if self.is_nai() {
            return Self::NAI;
        }
        let mapped_min = op(self.min);
        let mapped_max = op(self.max);
        debug_assert!(
            !mapped_min.is_nan() && !mapped_max.is_nan(),
            "monotonic operator produced NaN"
        );
        Self::encapsulating(mapped_min, mapped_max)
    }

    pub fn pow(self, exponent: Self) -> Self {
        if self.is_nai() || exponent.is_nai() {
            return Self::NAI;
        }
        if self.min == self.max {
            return Self::scalar_pow(self.min, exponent);
        }
        let mut result =
            Self::scalar_pow(self.min, exponent).union(Self::scalar_pow(self.max, exponent));
        if self.contains(0.0) {
            if self.max > 0.0 {
                result = result.union(Self::scalar_pow(0.0, exponent));
            }
            if self.min < 0.0 {
                result = result.union(Self::scalar_pow(-0.0, exponent));
            }
        }
        result
    }

    fn scalar_pow(base: f32, exponent: Self) -> Self {
        if base.is_nan() || exponent.is_nai() {
            return Self::NAI;
        }
        if exponent.min == exponent.max {
            let value = base.powf(exponent.min);
            return if value.is_nan() {
                Self::NAI
            } else {
                Self::exact(value)
            };
        }
        if base == 0.0 {
            return Self::zero_base_pow(exponent) * Self::exact(1.0f32.copysign(base));
        }
        if base == 1.0 {
            return Self::exact(1.0);
        }
        if base > 0.0 {
            Self::positive_base_pow(base, exponent)
        } else {
            Self::negative_base_pow(base, exponent)
        }
    }

    fn positive_base_pow(base: f32, exponent: Self) -> Self {
        if exponent.min.is_finite() && exponent.max.is_finite() {
            Self::encapsulating(base.powf(exponent.min), base.powf(exponent.max))
        } else if exponent.min.is_infinite() && exponent.max.is_infinite() {
            Self::of(0.0, f32::INFINITY)
        } else if exponent.min.is_infinite() {
            if base < 1.0 {
                Self::of(base.powf(exponent.max), f32::INFINITY)
            } else {
                Self::of(0.0, base.powf(exponent.max))
            }
        } else if base < 1.0 {
            Self::of(0.0, base.powf(exponent.min))
        } else {
            Self::of(base.powf(exponent.min), f32::INFINITY)
        }
    }

    fn zero_base_pow(exponent: Self) -> Self {
        if !exponent.contains(0.0) {
            return if exponent.max < 0.0 {
                Self::exact(f32::INFINITY)
            } else {
                Self::exact(0.0)
            };
        }
        if exponent.max == 0.0 {
            Self::of(1.0, f32::INFINITY)
        } else if exponent.min == 0.0 {
            Self::of(0.0, 1.0)
        } else {
            Self::of(0.0, f32::INFINITY)
        }
    }

    /// A negative base only yields a real value at integer exponents, so the
    /// hull is taken over the integers the exponent spans — and over their
    /// neighbours, because consecutive integers alternate in sign.
    fn negative_base_pow(base: f32, exponent: Self) -> Self {
        let exponent_min_int = exponent.min.ceil();
        let exponent_max_int = exponent.max.floor();
        if exponent_max_int < exponent_min_int {
            return Self::NAI;
        }
        let base_to_min_int = base.powf(exponent_min_int);
        let base_to_max_int = base.powf(exponent_max_int);
        let mut result = Self::encapsulating(base_to_min_int, base_to_max_int);
        if exponent_min_int.is_infinite() {
            result = result.union_value(-base_to_min_int);
        } else if exponent_min_int + 1.0 < exponent_max_int {
            result = result.union_value(base.powf(exponent_min_int + 1.0));
        }
        if exponent_max_int.is_infinite() {
            result = result.union_value(-base_to_max_int);
        } else if exponent_max_int - 1.0 > exponent_min_int {
            result = result.union_value(base.powf(exponent_max_int - 1.0));
        }
        result
    }

    pub fn lerp(alpha: Self, first: Self, second: Self) -> Self {
        if alpha.is_nai() || first.is_nai() || second.is_nai() {
            return Self::NAI;
        }
        Self::scalar_lerp(alpha, first.min, second.min)
            .union(Self::scalar_lerp(alpha, first.max, second.min))
            .union(Self::scalar_lerp(alpha, first.min, second.max))
            .union(Self::scalar_lerp(alpha, first.max, second.max))
    }

    fn scalar_lerp(alpha: Self, first: f32, second: f32) -> Self {
        if alpha.is_nai() || first.is_nan() || second.is_nan() {
            return Self::NAI;
        }
        if first.is_finite() && second.is_finite() {
            let bound = |a: f32| first + Self::mul_bound(a, second - first);
            return Self::encapsulating(bound(alpha.min), bound(alpha.max));
        }
        if first == second {
            return Self::exact(first);
        }
        let new_min = Self::infinite_lerp_bound(alpha.min, first, second);
        let new_max = Self::infinite_lerp_bound(alpha.max, first, second);
        if new_min.is_nan() || new_max.is_nan() {
            Self::NAI
        } else {
            Self::encapsulating(new_min, new_max)
        }
    }

    fn infinite_lerp_bound(alpha: f32, first: f32, second: f32) -> f32 {
        let first_part = Self::mul_bound(1.0 - alpha, first);
        let second_part = Self::mul_bound(alpha, second);
        if !first_part.is_infinite() || !second_part.is_infinite() {
            first_part + second_part
        } else if alpha <= 0.0 {
            if second > first {
                f32::NEG_INFINITY
            } else {
                f32::INFINITY
            }
        } else if alpha >= 1.0 {
            if second > first {
                f32::INFINITY
            } else {
                f32::NEG_INFINITY
            }
        } else {
            f32::NAN
        }
    }
}

impl std::ops::Add for Interval {
    type Output = Interval;

    fn add(self, other: Interval) -> Interval {
        Self::or_nai(self.min + other.min, self.max + other.max)
    }
}

impl std::ops::Sub for Interval {
    type Output = Interval;

    fn sub(self, other: Interval) -> Interval {
        Self::or_nai(self.min - other.max, self.max - other.min)
    }
}

impl std::ops::Mul for Interval {
    type Output = Interval;

    fn mul(self, other: Interval) -> Interval {
        self.multiplied(other)
    }
}

impl std::ops::Div for Interval {
    type Output = Interval;

    fn div(self, other: Interval) -> Interval {
        self.multiplied(other.reciprocal())
    }
}

#[cfg(test)]
mod test {
    use super::Interval;

    fn bounds(interval: Interval) -> (f32, f32) {
        (interval.min(), interval.max())
    }

    /// The two-corner shortcut this replaced answered -2 for the upper bound,
    /// excluding the reachable value 1 * -1.
    #[test]
    fn multiplication_takes_the_hull_of_all_four_corners() {
        let product = Interval::of(1.0, 2.0) * Interval::of(-3.0, -1.0);
        assert_eq!(bounds(product), (-6.0, -1.0));
    }

    #[test]
    fn a_zero_endpoint_absorbs_an_infinite_one() {
        assert_eq!(
            bounds(Interval::exact(0.0) * Interval::INFINITE),
            (0.0, 0.0)
        );
    }

    #[test]
    fn reciprocal_splits_on_where_zero_sits() {
        assert!(Interval::exact(0.0).reciprocal().is_nai());
        assert_eq!(
            bounds(Interval::symmetric(1.0).reciprocal()),
            (f32::NEG_INFINITY, f32::INFINITY)
        );
        assert_eq!(
            bounds(Interval::of(0.0, 2.0).reciprocal()),
            (0.5, f32::INFINITY)
        );
        assert_eq!(
            bounds(Interval::of(-2.0, 0.0).reciprocal()),
            (f32::NEG_INFINITY, -0.5)
        );
        assert_eq!(bounds(Interval::of(2.0, 4.0).reciprocal()), (0.25, 0.5));
    }

    #[test]
    fn a_negative_base_alternates_sign_across_the_integer_exponents() {
        let powers = Interval::exact(-2.0).pow(Interval::of(1.0, 3.0));
        assert_eq!(bounds(powers), (-8.0, 4.0));
        assert!(Interval::exact(-2.0).pow(Interval::of(0.25, 0.75)).is_nai());
    }

    #[test]
    fn a_zero_base_reaches_both_one_and_infinity() {
        assert_eq!(
            bounds(Interval::exact(0.0).pow(Interval::symmetric(1.0))),
            (0.0, f32::INFINITY)
        );
        assert_eq!(
            bounds(Interval::exact(0.0).pow(Interval::exact(-1.0))),
            (f32::INFINITY, f32::INFINITY)
        );
        assert_eq!(
            bounds(Interval::exact(0.5).pow(Interval::of(f32::NEG_INFINITY, 2.0))),
            (0.25, f32::INFINITY)
        );
    }

    #[test]
    fn lerping_towards_an_infinite_bound_stays_finite_at_alpha_zero() {
        let interpolated = Interval::lerp(
            Interval::of(0.0, 1.0),
            Interval::exact(0.0),
            Interval::exact(f32::INFINITY),
        );
        assert_eq!(bounds(interpolated), (0.0, f32::INFINITY));
    }

    #[test]
    fn log_clips_at_zero_and_gives_up_below_it() {
        assert_eq!(
            bounds(Interval::of(-1.0, 1.0).log()),
            (f32::NEG_INFINITY, 0.0)
        );
        assert!(Interval::of(-5.0, -1.0).log().is_nai());
    }

    #[test]
    fn clamping_past_a_bound_collapses_to_that_bound() {
        assert_eq!(
            bounds(Interval::of(5.0, 10.0).clamped(0.0, 3.0)),
            (3.0, 3.0)
        );
        assert_eq!(
            bounds(Interval::of(-10.0, -5.0).clamped(0.0, 3.0)),
            (0.0, 0.0)
        );
        assert_eq!(
            bounds(Interval::of(-1.0, 10.0).clamped(0.0, 3.0)),
            (0.0, 3.0)
        );
    }

    #[test]
    fn no_information_survives_every_operation() {
        assert!((Interval::NAI + Interval::exact(1.0)).is_nai());
        assert!((Interval::NAI * Interval::exact(1.0)).is_nai());
        assert!(Interval::NAI.abs().is_nai());
        assert!(Interval::NAI.sign().is_nai());
        assert_eq!(
            bounds(Interval::NAI.union(Interval::exact(2.0))),
            (2.0, 2.0)
        );
    }

    #[test]
    fn sign_keeps_the_zero_the_sampler_returns() {
        assert_eq!(bounds(Interval::of(0.0, 5.0).sign()), (0.0, 1.0));
        assert_eq!(bounds(Interval::of(-5.0, 0.0).sign()), (-1.0, 0.0));
        assert_eq!(bounds(Interval::symmetric(5.0).sign()), (-1.0, 1.0));
        assert_eq!(bounds(Interval::exact(0.0).sign()), (0.0, 0.0));
    }
}

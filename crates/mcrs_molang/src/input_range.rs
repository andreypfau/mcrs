/// The closed range of the values that an input or an intermediate value of a program can take.
/// The bounds can be infinite after an overflow; a range does not track NaN.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InputRange {
    pub min: f32,
    pub max: f32,
}

impl InputRange {
    /// Every finite value: the range of a pose input whose range the host does not know.
    pub const FINITE: Self = Self::new(f32::MIN, f32::MAX);
    /// Every value, the infinities included.
    pub const ANY: Self = Self::new(f32::NEG_INFINITY, f32::INFINITY);
    /// The Boolean values `0.0` and `1.0`.
    pub const BOOLEAN: Self = Self::new(0.0, 1.0);
    /// The range of no value, the start of a union.
    pub(crate) const EMPTY: Self = Self::new(f32::INFINITY, f32::NEG_INFINITY);

    #[must_use]
    pub const fn new(min: f32, max: f32) -> Self {
        Self { min, max }
    }

    #[must_use]
    pub const fn point(value: f32) -> Self {
        Self::new(value, value)
    }

    /// The range of the values of `self` and `other`.
    #[must_use]
    pub fn union(self, other: Self) -> Self {
        Self::new(self.min.min(other.min), self.max.max(other.max))
    }

    #[must_use]
    pub fn contains(self, value: f32) -> bool {
        self.min <= value && value <= self.max
    }

    /// The range of a function that does not decrease, from the images of the bounds. Rounding to
    /// nearest does not decrease either, so the images bound every rounded result.
    pub(crate) fn monotone(self, function: impl Fn(f32) -> f32) -> Self {
        Self::from_corners(&[function(self.min), function(self.max)])
    }

    /// The smallest range that holds every corner; a NaN corner gives [`InputRange::ANY`].
    pub(crate) fn from_corners(corners: &[f32]) -> Self {
        let mut range = Self::EMPTY;
        for corner in corners {
            if corner.is_nan() {
                return Self::ANY;
            }
            range = range.union(Self::point(*corner));
        }
        range
    }

    pub(crate) fn largest_magnitude(self) -> f32 {
        self.min.abs().max(self.max.abs())
    }

    pub(crate) fn add(self, other: Self) -> Self {
        Self::from_corners(&[self.min + other.min, self.max + other.max])
    }

    pub(crate) fn subtract(self, other: Self) -> Self {
        Self::from_corners(&[self.min - other.max, self.max - other.min])
    }

    pub(crate) fn multiply(self, other: Self) -> Self {
        Self::from_corners(&[
            self.min * other.min,
            self.min * other.max,
            self.max * other.min,
            self.max * other.max,
        ])
    }

    /// The range of `self / other` with the result `0.0` for a zero divisor.
    pub(crate) fn divide(self, other: Self) -> Self {
        if other.contains(0.0) {
            return Self::ANY;
        }
        Self::from_corners(&[
            self.min / other.min,
            self.min / other.max,
            self.max / other.min,
            self.max / other.max,
        ])
    }

    pub(crate) fn negate(self) -> Self {
        Self::new(-self.max, -self.min)
    }

    pub(crate) fn abs(self) -> Self {
        if self.min >= 0.0 {
            self
        } else if self.max <= 0.0 {
            self.negate()
        } else {
            Self::new(0.0, self.largest_magnitude())
        }
    }

    pub(crate) fn min_with(self, other: Self) -> Self {
        Self::new(self.min.min(other.min), self.max.min(other.max))
    }

    pub(crate) fn max_with(self, other: Self) -> Self {
        Self::new(self.min.max(other.min), self.max.max(other.max))
    }

    /// The range of a value with the sign of an unknown value and a magnitude from `self`.
    pub(crate) fn any_sign(self) -> Self {
        let magnitude = self.largest_magnitude();
        Self::new(-magnitude, magnitude)
    }

    /// The range of the remainder of a division of `self` with the sign of `self`.
    pub(crate) fn remainder(self, divisor: Self) -> Self {
        let bound = self.largest_magnitude().min(divisor.largest_magnitude());
        if self.min >= 0.0 {
            Self::new(0.0, bound)
        } else if self.max <= 0.0 {
            Self::new(-bound, 0.0)
        } else {
            Self::new(-bound, bound)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InputRange {
    pub min: f32,
    pub max: f32,
}

impl InputRange {
    pub const FINITE: Self = Self::new(f32::MIN, f32::MAX);
    pub const ANY: Self = Self::new(f32::NEG_INFINITY, f32::INFINITY);
    pub const BOOLEAN: Self = Self::new(0.0, 1.0);
    pub(crate) const EMPTY: Self = Self::new(f32::INFINITY, f32::NEG_INFINITY);

    #[must_use]
    pub const fn new(min: f32, max: f32) -> Self {
        Self { min, max }
    }

    #[must_use]
    pub const fn point(value: f32) -> Self {
        Self::new(value, value)
    }

    #[must_use]
    pub fn union(self, other: Self) -> Self {
        Self::new(self.min.min(other.min), self.max.max(other.max))
    }

    #[must_use]
    pub fn contains(self, value: f32) -> bool {
        self.min <= value && value <= self.max
    }

    pub(crate) fn monotone(self, function: impl Fn(f32) -> f32) -> Self {
        Self::from_corners(&[function(self.min), function(self.max)])
    }

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

    pub(crate) fn any_sign(self) -> Self {
        let magnitude = self.largest_magnitude();
        Self::new(-magnitude, magnitude)
    }

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

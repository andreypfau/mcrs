use core::ops::{Add, Div, Mul, Sub};

/// An IEEE 754 float in which [`lerp`] and [`remap_clamped`] compute.
pub trait Float:
    Copy
    + PartialOrd
    + Add<Output = Self>
    + Sub<Output = Self>
    + Mul<Output = Self>
    + Div<Output = Self>
{
    /// The value `0`.
    const ZERO: Self;
    /// The value `1`.
    const ONE: Self;
}

impl Float for f32 {
    const ZERO: Self = 0.0;
    const ONE: Self = 1.0;
}

impl Float for f64 {
    const ZERO: Self = 0.0;
    const ONE: Self = 1.0;
}

use std::fmt::Display;
use std::ops::Deref;

/// A newtype wrapper for `T` whose wire encoding is bounded by some upper
/// limit `MAX`; the wire impls are expected to error eagerly if the limit is
/// exceeded.
///
/// What exactly `MAX` represents depends on the type `T`. Here are some
/// instances:
/// - **arrays/slices**: The maximum number of elements.
/// - **strings**: The maximum number of utf16 code units.
/// - **raw bytes**: The maximum number of bytes.
#[derive(Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct Bounded<T, const MAX: usize>(pub T);

impl<T, const MAX: usize> Bounded<T, MAX> {
    pub fn map_into<U: From<T>>(self) -> Bounded<U, MAX> {
        Bounded(self.0.into())
    }
}

impl<T, const MAX: usize> Deref for Bounded<T, MAX> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T, const MAX: usize> From<T> for Bounded<T, MAX> {
    fn from(value: T) -> Self {
        Bounded(value)
    }
}

impl<T: Display, const MAX: usize> Display for Bounded<T, MAX> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

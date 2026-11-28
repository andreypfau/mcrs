use core::num::NonZeroU32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct Settings {
    pub max_age: Option<NonZeroU32>,
}

impl Settings {
    #[must_use]
    pub fn new(max_age: Option<NonZeroU32>) -> Self {
        Self { max_age }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self::new(NonZeroU32::new(600))
    }
}

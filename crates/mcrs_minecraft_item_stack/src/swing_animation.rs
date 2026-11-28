use crate::SwingKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SwingAnimation {
    pub kind: SwingKind,
    pub duration: i32,
}

impl SwingAnimation {
    pub const DEFAULT_KIND: SwingKind = SwingKind::Whack;
    pub const DEFAULT_DURATION: i32 = 6;
    pub const DEFAULT: Self = Self {
        kind: Self::DEFAULT_KIND,
        duration: Self::DEFAULT_DURATION,
    };
}

impl Default for SwingAnimation {
    fn default() -> Self {
        Self::DEFAULT
    }
}

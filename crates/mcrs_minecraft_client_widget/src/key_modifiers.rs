#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct KeyModifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
}

impl KeyModifiers {
    pub const NONE: Self = Self {
        shift: false,
        control: false,
        alt: false,
    };
    pub const SHIFT: Self = Self {
        shift: true,
        control: false,
        alt: false,
    };
    pub const CONTROL: Self = Self {
        shift: false,
        control: true,
        alt: false,
    };

    #[must_use]
    pub const fn is_shortcut(self) -> bool {
        self.control && !self.shift && !self.alt
    }
}

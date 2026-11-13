/// The modifier keys that are down during an input event.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct KeyModifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
}

impl KeyModifiers {
    /// No modifier key is down.
    pub const NONE: Self = Self {
        shift: false,
        control: false,
        alt: false,
    };
    /// Shift is down.
    pub const SHIFT: Self = Self {
        shift: true,
        control: false,
        alt: false,
    };
    /// Ctrl is down.
    pub const CONTROL: Self = Self {
        shift: false,
        control: true,
        alt: false,
    };

    /// Whether these modifiers make a shortcut such as select all, copy, paste or cut:
    /// Ctrl without Shift and Alt (EDIT-KEY-001).
    #[must_use]
    pub const fn is_shortcut(self) -> bool {
        self.control && !self.shift && !self.alt
    }
}

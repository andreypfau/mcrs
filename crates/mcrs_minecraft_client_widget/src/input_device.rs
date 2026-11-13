/// The last input device of the GUI (WID-IN-001).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum InputDevice {
    #[default]
    None,
    Mouse,
    Tab,
    Arrow,
    Activation,
}

impl InputDevice {
    /// Whether the device is a keyboard value: Tab, arrow or activation.
    #[must_use]
    pub const fn is_keyboard(self) -> bool {
        matches!(self, Self::Tab | Self::Arrow | Self::Activation)
    }
}

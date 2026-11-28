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
    #[must_use]
    pub const fn is_keyboard(self) -> bool {
        matches!(self, Self::Tab | Self::Arrow | Self::Activation)
    }
}

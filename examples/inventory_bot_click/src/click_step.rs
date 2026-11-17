use mcrs_minecraft_container_click::ButtonBinding;
use mcrs_minecraft_container_click::Gesture;

const LEFT: u8 = 0;
const RIGHT: u8 = 1;

/// One mouse click of the bot on one entry of the inventory menu, as a GUI sees it: the
/// button goes down and then up over the same entry, with or without the shift key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClickStep {
    pub entry: u16,
    pub button: u8,
    pub shift: bool,
}

impl ClickStep {
    #[must_use]
    pub const fn left(entry: u16) -> Self {
        Self::new(entry, LEFT)
    }

    #[must_use]
    pub const fn right(entry: u16) -> Self {
        Self::new(entry, RIGHT)
    }

    #[must_use]
    pub const fn shift_left(entry: u16) -> Self {
        Self {
            shift: true,
            ..Self::left(entry)
        }
    }

    #[must_use]
    pub const fn press(self) -> Gesture {
        Gesture::Press {
            button: self.button,
            binding: ButtonBinding::Other,
        }
    }

    #[must_use]
    pub const fn release(self) -> Gesture {
        Gesture::Release {
            button: self.button,
            binding: ButtonBinding::Other,
        }
    }

    const fn new(entry: u16, button: u8) -> Self {
        Self {
            entry,
            button,
            shift: false,
        }
    }
}

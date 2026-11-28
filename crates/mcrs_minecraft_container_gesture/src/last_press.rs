use core::time::Duration;

use crate::gesture_button::GestureButton;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LastPress {
    pub(crate) button: GestureButton,
    pub(crate) target: Option<u16>,
    pub(crate) at: Duration,
}

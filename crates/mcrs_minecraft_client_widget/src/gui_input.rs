use crate::gui_point::GuiPoint;
use crate::input_device::InputDevice;

const MOUSE_TAKEOVER_DISTANCE: i64 = 7;

/// The last input device, its anchor and the mouse position: one value for the client, which a
/// new screen takes from the screen before it (WID-IN-001).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GuiInput {
    device: InputDevice,
    anchor: GuiPoint,
    mouse: GuiPoint,
}

impl GuiInput {
    /// The input of a screen that opens while no screen is open: no device, the mouse at `mouse`.
    #[must_use]
    pub const fn new(mouse: GuiPoint) -> Self {
        Self {
            device: InputDevice::None,
            anchor: mouse,
            mouse,
        }
    }

    /// The last input device.
    #[must_use]
    pub const fn device(self) -> InputDevice {
        self.device
    }

    /// The mouse position in GUI units.
    #[must_use]
    pub const fn mouse(self) -> GuiPoint {
        self.mouse
    }

    pub(crate) fn set_device(&mut self, device: InputDevice) {
        if self.device != device {
            self.device = device;
            self.anchor = self.mouse;
        }
    }

    pub(crate) fn move_mouse(&mut self, mouse: GuiPoint) {
        self.mouse = mouse;
        let across = i64::from(mouse.x) - i64::from(self.anchor.x);
        let down = i64::from(mouse.y) - i64::from(self.anchor.y);
        let squared = across * across + down * down;
        if squared > MOUSE_TAKEOVER_DISTANCE * MOUSE_TAKEOVER_DISTANCE {
            self.set_device(InputDevice::Mouse);
        }
    }
}

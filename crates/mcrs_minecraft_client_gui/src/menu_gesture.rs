use core::time::Duration;

use mcrs_minecraft_item_stack::ItemStack;

use crate::last_press::LastPress;
use crate::open_drag::OpenDrag;
use crate::press_kind::PressKind;
use crate::release_rule::ReleaseRule;

const DOUBLE_CLICK: Duration = Duration::from_millis(250);

#[derive(Default, Debug)]
pub(crate) struct MenuGesture {
    pub(crate) release: ReleaseRule,
    pub(crate) press: PressKind,
    last_press: Option<LastPress>,
    pub(crate) remembered: Option<ItemStack>,
}

impl MenuGesture {
    pub(crate) fn open_drag(&self) -> Option<OpenDrag> {
        match self.release {
            ReleaseRule::Drag(open) => Some(open),
            ReleaseRule::NoClick | ReleaseRule::Click => None,
        }
    }

    pub(crate) fn take_press(&mut self, press: LastPress) {
        let double = self.last_press.is_some_and(|last| {
            press.at.saturating_sub(last.at) < DOUBLE_CLICK
                && last.button == press.button
                && last.menu == press.menu
                && last.target == press.target
        });
        self.press = if double {
            PressKind::Double
        } else {
            PressKind::Single
        };
        self.last_press = Some(press);
    }
}

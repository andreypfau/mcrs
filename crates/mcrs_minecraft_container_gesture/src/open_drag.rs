use mcrs_minecraft_inventory_click::DragKind;

use crate::gesture_button::GestureButton;
use crate::press_kind::PressKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct OpenDrag {
    kind: DragKind,
    opened_by: PressKind,
    button: GestureButton,
    entries: u128,
}

impl OpenDrag {
    pub(crate) fn new(kind: DragKind, opened_by: PressKind, button: GestureButton) -> Self {
        Self {
            kind,
            opened_by,
            button,
            entries: 0,
        }
    }

    pub(crate) fn kind(self) -> DragKind {
        self.kind
    }

    pub(crate) fn opened_by(self) -> PressKind {
        self.opened_by
    }

    pub(crate) fn button(self) -> GestureButton {
        self.button
    }

    pub(crate) fn len(self) -> u32 {
        self.entries.count_ones()
    }

    pub(crate) fn contains(self, index: u16) -> bool {
        self.entries & bit(index) != 0
    }

    pub(crate) fn insert(&mut self, index: u16) {
        self.entries |= bit(index);
    }

    pub(crate) fn remove(&mut self, index: u16) {
        self.entries &= !bit(index);
    }

    pub(crate) fn entries(self) -> impl Iterator<Item = u16> {
        (0..u128::BITS as u16).filter(move |index| self.contains(*index))
    }
}

fn bit(index: u16) -> u128 {
    1u128.checked_shl(u32::from(index)).unwrap_or(0)
}

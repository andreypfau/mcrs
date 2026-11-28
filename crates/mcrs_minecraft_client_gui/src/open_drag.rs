use bevy::prelude::*;
use mcrs_minecraft_inventory_click::DragKind;

use crate::press_kind::PressKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct OpenDrag {
    pub(crate) kind: DragKind,
    pub(crate) opened_by: PressKind,
    entries: u128,
}

impl OpenDrag {
    pub(crate) fn new(kind: DragKind, opened_by: PressKind) -> Self {
        Self {
            kind,
            opened_by,
            entries: 0,
        }
    }

    pub(crate) fn button(self) -> MouseButton {
        match self.kind {
            DragKind::Split => MouseButton::Left,
            DragKind::Single => MouseButton::Right,
            DragKind::Full => MouseButton::Middle,
        }
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

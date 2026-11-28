use mcrs_minecraft_container_click::StackPresence;
use mcrs_minecraft_inventory_click::{DragEntry, DragKind, DragSplit};

pub trait MenuView {
    type Stack: Clone;

    fn carried(&self) -> StackPresence;

    fn entry(&self, index: u16) -> StackPresence;

    fn knows_drag_rules(&self) -> bool {
        false
    }

    fn joins_drag(&self, _index: u16) -> Option<DragEntry> {
        None
    }

    fn stays_in_drag(&self, _index: u16) -> Option<DragEntry> {
        None
    }

    fn drag_split(&self, _kind: DragKind, _size: u32) -> Option<DragSplit> {
        None
    }

    fn permits_pick_up_all(&self, _index: u16) -> bool {
        false
    }

    fn entry_count(&self) -> u16 {
        0
    }

    fn stack(&mut self, _index: u16) -> Option<Self::Stack> {
        None
    }

    fn shift_double_click_moves(
        &mut self,
        _target: u16,
        _index: u16,
        _remembered: &mut Self::Stack,
    ) -> bool {
        false
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NoMenu;

impl MenuView for NoMenu {
    type Stack = ();

    fn carried(&self) -> StackPresence {
        StackPresence::Empty
    }

    fn entry(&self, _index: u16) -> StackPresence {
        StackPresence::Empty
    }
}

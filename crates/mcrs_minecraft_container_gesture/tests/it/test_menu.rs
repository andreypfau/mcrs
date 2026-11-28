use mcrs_minecraft_container_click::{Clicked, StackPresence};
use mcrs_minecraft_container_gesture::{
    GestureClick, GestureFrame, GestureHit, MenuGesture, MenuView, Viewer,
};
use mcrs_minecraft_inventory_click::{DragEntry, DragKind, DragSplit};

#[derive(Clone, Copy, Debug)]
pub(crate) struct TestMenu {
    pub(crate) carried: StackPresence,
    pub(crate) left: &'static [u16],
}

pub(crate) const EMPTY_HANDED: TestMenu = TestMenu {
    carried: StackPresence::Empty,
    left: &[],
};

const ROOM: DragEntry = DragEntry { held: 0, cap: 64 };

impl MenuView for TestMenu {
    type Stack = ();

    fn carried(&self) -> StackPresence {
        self.carried
    }

    fn entry(&self, _index: u16) -> StackPresence {
        StackPresence::Stack
    }

    fn knows_drag_rules(&self) -> bool {
        true
    }

    fn joins_drag(&self, _index: u16) -> Option<DragEntry> {
        Some(ROOM)
    }

    fn stays_in_drag(&self, index: u16) -> Option<DragEntry> {
        (!self.left.contains(&index)).then_some(ROOM)
    }

    fn drag_split(&self, kind: DragKind, size: u32) -> Option<DragSplit> {
        Some(DragSplit {
            kind,
            count: 64,
            max: 64,
            size,
        })
    }

    fn permits_pick_up_all(&self, _index: u16) -> bool {
        true
    }
}

pub(crate) fn frame<'a>(
    menu: TestMenu,
    gesture: &'a mut MenuGesture<()>,
    sent: &'a mut Vec<GestureClick>,
) -> GestureFrame<'a, TestMenu> {
    GestureFrame {
        viewer: Viewer {
            menu,
            creative: false,
        },
        gesture,
        sent,
    }
}

pub(crate) fn on_entry(index: u16) -> GestureHit {
    GestureHit {
        clicked: Some(Clicked::Entry(index)),
        entry: Some(index),
        target: None,
        takes_left_press: false,
        permits_drag: true,
    }
}

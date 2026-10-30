use crate::edit_box::{Anchor, Step};
use crate::tab_direction::TabDirection;

/// A key of the chat screen, after the platform mapped Cmd or Ctrl.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Up,
    Down,
    Tab { direction: TabDirection },
    Escape,
    Enter,
    Backspace { step: Step },
    Delete { step: Step },
    Left { step: Step, anchor: Anchor },
    Right { step: Step, anchor: Anchor },
    Home { anchor: Anchor },
    End { anchor: Anchor },
    PageUp,
    PageDown,
    SelectAll,
    Copy,
    Cut,
}

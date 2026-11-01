use bevy::window::{CursorGrabMode, CursorOptions};

pub(crate) fn grab(cursor: &mut CursorOptions, grabbed: bool) {
    cursor.grab_mode = if grabbed {
        CursorGrabMode::Locked
    } else {
        CursorGrabMode::None
    };
    cursor.visible = !grabbed;
}

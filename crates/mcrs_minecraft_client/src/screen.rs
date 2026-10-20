use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions};
use mcrs_minecraft_protocol::GameMode;

#[derive(Resource, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    #[default]
    None,
    Inventory,
    GameModeSwitcher(GameMode),
}

pub(crate) fn grab(cursor: &mut CursorOptions, grabbed: bool) {
    cursor.grab_mode = if grabbed {
        CursorGrabMode::Locked
    } else {
        CursorGrabMode::None
    };
    cursor.visible = !grabbed;
}

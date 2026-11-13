use bevy::prelude::*;
use mcrs_minecraft_game_mode::GameMode;

#[derive(Resource, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    #[default]
    None,
    Inventory,
    GameModeSwitcher(GameMode),
    Chat,
    Pause,
    Options,
    VideoSettings,
    AccessibilitySettings,
    /// "Controls", which Escape and "Done" close to its parent screen.
    Controls(ControlsParent),
    /// "Mouse Settings", which Escape and "Done" close to "Controls" with the same parent screen.
    MouseSettings(ControlsParent),
}

/// The screen that opened "Controls".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlsParent {
    Options,
    AccessibilitySettings,
}

use bevy::prelude::*;
use mcrs_minecraft_game_mode::GameMode;

#[derive(Resource, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    #[default]
    None,
    Title,
    Menu,
    GameModeSwitcher(GameMode),
    Chat,
    Pause,
    Options,
    VideoSettings,
    AccessibilitySettings(AccessibilityParent),
    Controls(ControlsParent),
    MouseSettings(ControlsParent),
    SelectWorld,
    WorldListError,
    DeleteWorldConfirmation,
    CreateWorld(CreateWorldParent),
    Message(ScreenMessage),
    LevelLoading,
    Disconnected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScreenMessage {
    ReadingWorldData,
    PreparingWorldCreation,
    SavingWorld,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreateWorldParent {
    SelectWorld,
    Title,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlsParent {
    Options,
    AccessibilitySettings(AccessibilityParent),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessibilityParent {
    Options,
    Title,
}

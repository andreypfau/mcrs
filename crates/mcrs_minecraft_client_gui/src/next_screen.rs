use mcrs_minecraft_client_widget::{WidgetEvent, WidgetId};
use mcrs_minecraft_client_world::screen::{
    AccessibilityParent, ControlsParent, CreateWorldParent, Screen,
};

use crate::accessibility_settings_screen;
use crate::controls_screen;
use crate::create_world_screen;
use crate::delete_world_screen;
use crate::disconnected_screen;
use crate::menu_event::MenuEvent;
use crate::option_sub_screen;
use crate::options_screen;
use crate::pause_screen;
use crate::select_world_screen;
use crate::title_screen;
use crate::world_list_error_screen;
use crate::world_presence::WorldPresence;

fn parent(screen: Screen, world: WorldPresence) -> Screen {
    match screen {
        Screen::Options => match world {
            WorldPresence::InWorld => Screen::Pause,
            WorldPresence::NoWorld => Screen::Title,
        },
        Screen::AccessibilitySettings(AccessibilityParent::Title) => Screen::Title,
        Screen::VideoSettings
        | Screen::AccessibilitySettings(AccessibilityParent::Options)
        | Screen::Controls(ControlsParent::Options) => Screen::Options,
        Screen::Controls(ControlsParent::AccessibilitySettings(parent)) => {
            Screen::AccessibilitySettings(parent)
        }
        Screen::MouseSettings(parent) => Screen::Controls(parent),
        Screen::SelectWorld | Screen::CreateWorld(CreateWorldParent::Title) => Screen::Title,
        Screen::DeleteWorldConfirmation | Screen::CreateWorld(CreateWorldParent::SelectWorld) => {
            Screen::SelectWorld
        }
        Screen::None
        | Screen::Title
        | Screen::Menu
        | Screen::GameModeSwitcher(_)
        | Screen::Chat
        | Screen::Pause
        | Screen::Death
        | Screen::Message(_)
        | Screen::LevelLoading
        | Screen::Disconnected
        | Screen::WorldListError => Screen::None,
    }
}

fn target(shown: Screen, widget: WidgetId, world: WorldPresence) -> Option<Screen> {
    match (shown, widget) {
        (Screen::Pause, pause_screen::OPTIONS) | (Screen::Title, title_screen::OPTIONS) => {
            Some(Screen::Options)
        }
        (Screen::Title, title_screen::ACCESSIBILITY) => {
            Some(Screen::AccessibilitySettings(AccessibilityParent::Title))
        }
        (Screen::Options, options_screen::VIDEO) => Some(Screen::VideoSettings),
        (Screen::Options, options_screen::ACCESSIBILITY) => {
            Some(Screen::AccessibilitySettings(AccessibilityParent::Options))
        }
        (Screen::Options, options_screen::CONTROLS) => {
            Some(Screen::Controls(ControlsParent::Options))
        }
        (Screen::AccessibilitySettings(parent), accessibility_settings_screen::CONTROLS) => Some(
            Screen::Controls(ControlsParent::AccessibilitySettings(parent)),
        ),
        (Screen::Controls(parent), controls_screen::MOUSE_SETTINGS) => {
            Some(Screen::MouseSettings(parent))
        }
        (Screen::Disconnected, disconnected_screen::BACK)
        | (Screen::WorldListError, world_list_error_screen::CANCEL)
        | (Screen::SelectWorld, select_world_screen::BACK) => Some(Screen::Title),
        (Screen::Title, title_screen::SINGLEPLAYER) => Some(Screen::SelectWorld),
        (Screen::SelectWorld, select_world_screen::DELETE) => Some(Screen::DeleteWorldConfirmation),
        (Screen::SelectWorld, select_world_screen::CREATE) => {
            Some(Screen::CreateWorld(CreateWorldParent::SelectWorld))
        }
        (
            Screen::DeleteWorldConfirmation,
            delete_world_screen::DELETE | delete_world_screen::CANCEL,
        ) => Some(Screen::SelectWorld),
        (Screen::Pause, pause_screen::BACK_TO_GAME)
        | (Screen::Options, options_screen::DONE)
        | (Screen::CreateWorld(_), create_world_screen::CANCEL)
        | (
            Screen::VideoSettings
            | Screen::AccessibilitySettings(_)
            | Screen::Controls(_)
            | Screen::MouseSettings(_),
            option_sub_screen::DONE,
        ) => Some(parent(shown, world)),
        _ => None,
    }
}

pub(crate) fn next_screen(shown: Screen, event: MenuEvent, world: WorldPresence) -> Option<Screen> {
    match event {
        MenuEvent::CloseRequested
            if matches!(
                shown,
                Screen::Title
                    | Screen::Death
                    | Screen::Message(_)
                    | Screen::LevelLoading
                    | Screen::Disconnected
                    | Screen::WorldListError
            ) =>
        {
            None
        }
        MenuEvent::CloseRequested => Some(parent(shown, world)),
        MenuEvent::Widget(WidgetEvent::Activated { widget, .. }) => target(shown, widget, world),
        MenuEvent::Widget(_) | MenuEvent::Confirmed => None,
    }
}

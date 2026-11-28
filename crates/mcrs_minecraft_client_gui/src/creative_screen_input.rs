use core::time::Duration;

use mcrs_minecraft_client_widget::{Anchor, DOUBLE_CLICK_MS, GuiKey};
use mcrs_minecraft_container_gesture::Modifiers;
use mcrs_minecraft_menu_geometry::CreativeHit;

use crate::gesture_input::GestureInput;
use crate::hover::Hover;
use crate::item_key_codes;
use crate::menu_input;
use crate::search_input::SearchInput;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ScrollDrag {
    #[default]
    Idle,
    Dragging,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SearchPress {
    #[default]
    None,
    Held(Duration),
    Released(Duration),
}

impl SearchPress {
    fn at(self) -> Option<Duration> {
        match self {
            Self::None => None,
            Self::Held(at) | Self::Released(at) => Some(at),
        }
    }
}

pub(crate) fn search_press(
    search: &mut SearchPress,
    at: Duration,
    modifiers: Modifiers,
) -> SearchInput {
    let double = search
        .at()
        .is_some_and(|last| at.saturating_sub(last) <= Duration::from_millis(DOUBLE_CLICK_MS));
    *search = SearchPress::Held(at);
    if double {
        SearchInput::SelectWord
    } else if modifiers.shift {
        SearchInput::Click(Anchor::Keep)
    } else {
        SearchInput::Click(Anchor::Move)
    }
}

pub(crate) fn swaps_from_search(
    frame: &mut GestureInput,
    input: &SearchInput,
    hover: Hover,
    modifiers: Modifiers,
) -> bool {
    let SearchInput::Key { input, .. } = input else {
        return false;
    };
    if !matches!(menu_input::gui_key(input.key_code), Some(GuiKey::Digit(_))) {
        return false;
    }
    let Some(gesture) = item_key_codes::key_gesture(input.key_code) else {
        return false;
    };
    if let Hover::Creative(CreativeHit::GridSlot(slot)) = hover
        && frame.grid.and_then(|grid| grid.stack(slot)).is_none()
    {
        return false;
    }
    frame.click(hover, gesture, modifiers)
}

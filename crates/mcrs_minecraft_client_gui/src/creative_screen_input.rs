use core::time::Duration;

use bevy::math::DVec2;
use bevy::prelude::*;
use mcrs_minecraft_client_widget::{Anchor, DOUBLE_CLICK_MS, GuiKey, GuiRect};
use mcrs_minecraft_menu_geometry::{CreativeGeometry, CreativeHit};

use crate::creative_screen_draw;
use crate::creative_screen_edit::CreativeScreenEdit;
use crate::gesture_frame::GestureFrame;
use crate::hover::Hover;
use crate::inventory_clicks::{self, ScreenEvent};
use crate::menu_input;
use crate::modifiers::Modifiers;
use crate::scene::{self, GuiConfig, GuiSize};
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

#[derive(Clone, Copy, Debug)]
pub(crate) struct CreativePointer {
    pub(crate) top: i32,
    pub(crate) search_box: GuiRect,
    pub(crate) mouse: Option<DVec2>,
}

pub(crate) fn creative_pointer(window: &Window, config: &GuiConfig) -> CreativePointer {
    let size = GuiSize::of(scene::window_framebuffer(window), config.scale);
    let corner = CreativeGeometry::window(size.width, size.height);
    CreativePointer {
        top: corner.y,
        search_box: creative_screen_draw::offset(corner, CreativeGeometry::SEARCH_BOX),
        mouse: scene::gui_pointer(window, config, size.scale),
    }
}

pub(crate) fn follow_pointer(
    scroll: ScrollDrag,
    search: SearchPress,
    at: CreativePointer,
    tabs: &mut CreativeScreenEdit,
) {
    let Some(mouse) = at.mouse else {
        return;
    };
    if scroll == ScrollDrag::Dragging {
        tabs.drag_scroll(at.top, mouse.y);
    }
    if matches!(search, SearchPress::Held(_)) {
        tabs.edit_search(&SearchInput::Drag, Some(at));
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
    frame: &mut GestureFrame,
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
    let Some(ScreenEvent::Key(gesture)) = inventory_clicks::key_event(input.key_code) else {
        return false;
    };
    if let Hover::Creative(CreativeHit::GridSlot(slot)) = hover
        && frame.grid.and_then(|grid| grid.stack(slot)).is_none()
    {
        return false;
    }
    frame.click(hover, gesture, modifiers)
}

use bevy::clipboard::Clipboard;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::time::Real;
use mcrs_minecraft_menu_geometry::{CreativeTabIndex, CreativeTabKind};

use crate::creative_pointer::CreativePointer;
use crate::creative_screen_state::CreativeScreenState;
use crate::creative_search_edit;
use crate::creative_tab_content::CreativeTabContent;
use crate::edit_input;
use crate::layer_due;
use crate::menu_input;
use crate::search_input::SearchInput;
use mcrs_minecraft_font::font::Font;

#[derive(SystemParam)]
pub(crate) struct CreativeScreenEdit<'w> {
    screen: Option<ResMut<'w, CreativeScreenState>>,
    content: Option<Res<'w, CreativeTabContent>>,
    clipboard: Option<ResMut<'w, Clipboard>>,
    font: Option<Res<'w, Font>>,
    time: Option<Res<'w, Time<Real>>>,
}

impl CreativeScreenEdit<'_> {
    pub(crate) fn select(&mut self, tab: CreativeTabIndex) {
        let now_ms = self.time.as_deref().map(layer_due::gui_clock);
        self.with(|screen, content| {
            screen.select(tab, content);
            if let Some(now_ms) = now_ms
                && screen.selected_tab().kind == CreativeTabKind::Search
            {
                screen.focus_search(now_ms);
            }
        });
    }

    pub(crate) fn wheel(&mut self, vertical: f64) {
        self.with(|screen, content| screen.wheel(vertical, content));
    }

    pub(crate) fn drag_scroll(&mut self, top: i32, mouse_y: f64) {
        self.with(|screen, content| screen.drag_scroll(top, mouse_y, content));
    }

    fn with(&mut self, change: impl FnOnce(&mut CreativeScreenState, &CreativeTabContent)) {
        if let (Some(screen), Some(content)) = (self.screen.as_mut(), self.content.as_deref()) {
            change(screen, content);
        }
    }

    pub(crate) fn edit_search(&mut self, input: &SearchInput, at: CreativePointer) {
        let (Some(screen), Some(content)) = (self.screen.as_mut(), self.content.as_deref()) else {
            return;
        };
        let mut clipboard = self.clipboard.as_deref_mut();
        let width = menu_input::text_width(self.font.as_deref());
        let mut copied = None;
        screen.edit_search(content, |edit| {
            copied = creative_search_edit::apply_search(
                edit,
                input,
                at.search_box,
                at.mouse.x,
                &width,
                || edit_input::clipboard_text(clipboard.as_deref_mut()),
            );
        });
        if let Some(text) = copied {
            edit_input::copy_text(clipboard, &text);
        }
    }
}

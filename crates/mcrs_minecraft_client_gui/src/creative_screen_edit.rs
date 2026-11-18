use bevy::clipboard::Clipboard;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use mcrs_minecraft_menu_geometry::CreativeTabIndex;

use crate::chat_input;
use crate::creative_screen_input::CreativePointer;
use crate::creative_screen_state::CreativeScreenState;
use crate::creative_search_edit;
use crate::creative_tab_content::CreativeTabContent;
use crate::menu_input;
use crate::scene::GuiAtlas;
use crate::search_input::SearchInput;

#[derive(SystemParam)]
pub(crate) struct CreativeScreenEdit<'w> {
    screen: Option<ResMut<'w, CreativeScreenState>>,
    content: Option<Res<'w, CreativeTabContent>>,
    clipboard: Option<ResMut<'w, Clipboard>>,
    atlas: Option<Res<'w, GuiAtlas>>,
}

impl CreativeScreenEdit<'_> {
    pub(crate) fn select(&mut self, tab: CreativeTabIndex) {
        self.with(|screen, content| screen.select(tab, content));
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

    pub(crate) fn edit_search(&mut self, input: &SearchInput, at: Option<CreativePointer>) {
        let (Some(screen), Some(content), Some(at)) =
            (self.screen.as_mut(), self.content.as_deref(), at)
        else {
            return;
        };
        let mut clipboard = self.clipboard.as_deref_mut();
        let width = menu_input::text_width(self.atlas.as_deref());
        let mut copied = None;
        screen.edit_search(content, |edit| {
            let mouse_x = at.mouse.map(|mouse| mouse.x);
            copied = creative_search_edit::apply_search(
                edit,
                input,
                at.search_box,
                mouse_x,
                &width,
                || chat_input::clipboard_text(clipboard.as_deref_mut()),
            );
        });
        if let Some(text) = copied {
            chat_input::copy_text(clipboard, &text);
        }
    }
}

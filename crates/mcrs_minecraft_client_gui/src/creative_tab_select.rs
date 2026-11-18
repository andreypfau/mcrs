use bevy::clipboard::Clipboard;
use bevy::ecs::system::SystemParam;
use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::*;
use mcrs_minecraft_chat_client::Key;
use mcrs_minecraft_client_widget::{EditBox, KeyModifiers};
use mcrs_minecraft_menu_geometry::CreativeTabIndex;

use crate::chat_input;
use crate::creative_screen_state::CreativeScreenState;
use crate::creative_tab_content::CreativeTabContent;
use crate::search_input::SearchInput;

#[derive(SystemParam)]
pub(crate) struct CreativeTabSelect<'w> {
    screen: Option<ResMut<'w, CreativeScreenState>>,
    content: Option<Res<'w, CreativeTabContent>>,
    clipboard: Option<ResMut<'w, Clipboard>>,
}

impl CreativeTabSelect<'_> {
    pub(crate) fn select(&mut self, tab: CreativeTabIndex) {
        if let (Some(screen), Some(content)) = (self.screen.as_mut(), self.content.as_deref()) {
            screen.select(tab, content);
        }
    }

    pub(crate) fn wheel(&mut self, vertical: f64) {
        if let (Some(screen), Some(content)) = (self.screen.as_mut(), self.content.as_deref()) {
            screen.wheel(vertical, content);
        }
    }

    pub(crate) fn drag_scroll(&mut self, top: i32, mouse_y: f64) {
        if let (Some(screen), Some(content)) = (self.screen.as_mut(), self.content.as_deref()) {
            screen.drag_scroll(top, mouse_y, content);
        }
    }

    pub(crate) fn edit_search(&mut self, input: &SearchInput) {
        let (Some(screen), Some(content)) = (self.screen.as_mut(), self.content.as_deref()) else {
            return;
        };
        let clipboard = self.clipboard.as_deref_mut();
        screen.edit_search(content, |edit| match input {
            SearchInput::Key { input, modifiers } => type_key(edit, input, *modifiers, clipboard),
            SearchInput::SelectAll => edit.select_all(),
        });
    }
}

fn type_key(
    edit: &mut EditBox,
    input: &KeyboardInput,
    modifiers: KeyModifiers,
    clipboard: Option<&mut Clipboard>,
) {
    let Some(key) = chat_input::screen_key(input.key_code, modifiers) else {
        if chat_input::pastes(input.key_code, modifiers) {
            if let Some(text) = chat_input::clipboard_text(clipboard) {
                edit.insert(&text);
            }
        } else if let Some(text) = input.text.as_deref().filter(|_| !modifiers.control) {
            edit.insert(text);
        }
        return;
    };
    match key {
        Key::Backspace { step } => {
            edit.backspace(step);
        }
        Key::Delete { step } => {
            edit.delete(step);
        }
        Key::Left { step, anchor } => edit.left(step, anchor),
        Key::Right { step, anchor } => edit.right(step, anchor),
        Key::Home { anchor } => edit.home(anchor),
        Key::End { anchor } => edit.end(anchor),
        Key::SelectAll => edit.select_all(),
        Key::Copy => chat_input::copy_text(clipboard, edit.selected()),
        Key::Cut => {
            chat_input::copy_text(clipboard, edit.selected());
            edit.cut();
        }
        Key::Up
        | Key::Down
        | Key::Tab { .. }
        | Key::Escape
        | Key::Enter
        | Key::PageUp
        | Key::PageDown => {}
    }
}

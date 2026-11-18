use bevy::input::keyboard::KeyboardInput;
use mcrs_minecraft_chat_client::Key;
use mcrs_minecraft_client_widget::{EditBox, GuiRect, KeyModifiers};

use crate::chat_input;
use crate::search_input::SearchInput;

pub(crate) fn apply_search(
    edit: &mut EditBox,
    input: &SearchInput,
    search_box: GuiRect,
    mouse_x: Option<f64>,
    width: &impl Fn(&str) -> i32,
    paste: impl FnOnce() -> Option<String>,
) -> Option<String> {
    let copied = match (input, mouse_x) {
        (SearchInput::Key { input, modifiers }, _) => type_key(edit, input, *modifiers, paste),
        (SearchInput::SelectAll, _) => {
            edit.select_all();
            None
        }
        (SearchInput::Click(anchor), Some(x)) => {
            edit.click(search_box, x, *anchor, width);
            None
        }
        (SearchInput::SelectWord, Some(x)) => {
            edit.select_word(search_box, x, width);
            None
        }
        (SearchInput::Drag, Some(x)) => {
            edit.drag(search_box, x, width);
            None
        }
        (SearchInput::Click(_) | SearchInput::SelectWord | SearchInput::Drag, None) => None,
    };
    edit.scroll(search_box, width);
    copied
}

fn type_key(
    edit: &mut EditBox,
    input: &KeyboardInput,
    modifiers: KeyModifiers,
    paste: impl FnOnce() -> Option<String>,
) -> Option<String> {
    let Some(key) = chat_input::screen_key(input.key_code, modifiers) else {
        if chat_input::pastes(input.key_code, modifiers) {
            if let Some(text) = paste() {
                edit.insert(&text);
            }
        } else if let Some(text) = input.text.as_deref().filter(|_| !modifiers.control) {
            edit.insert(text);
        }
        return None;
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
        Key::Copy => return selected_text(edit),
        Key::Cut => {
            let selected = selected_text(edit);
            edit.cut();
            return selected;
        }
        Key::Up
        | Key::Down
        | Key::Tab { .. }
        | Key::Escape
        | Key::Enter
        | Key::PageUp
        | Key::PageDown => {}
    }
    None
}

fn selected_text(edit: &EditBox) -> Option<String> {
    let selected = edit.selected();
    (!selected.is_empty()).then(|| selected.to_owned())
}

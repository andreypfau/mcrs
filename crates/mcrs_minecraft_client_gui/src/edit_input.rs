use bevy::clipboard::{Clipboard, ClipboardRead};
use bevy::prelude::KeyCode;

use mcrs_minecraft_chat_client::{Key, TabDirection};
use mcrs_minecraft_client_widget::{Anchor, KeyModifiers, Step};

pub(crate) fn screen_key(code: KeyCode, modifiers: KeyModifiers) -> Option<Key> {
    let step = if modifiers.control {
        Step::Word
    } else {
        Step::Char
    };
    let anchor = if modifiers.shift {
        Anchor::Keep
    } else {
        Anchor::Move
    };
    Some(match code {
        KeyCode::ArrowUp => Key::Up,
        KeyCode::ArrowDown => Key::Down,
        KeyCode::Tab => Key::Tab {
            direction: if modifiers.shift {
                TabDirection::Backward
            } else {
                TabDirection::Forward
            },
        },
        KeyCode::Escape => Key::Escape,
        KeyCode::Enter | KeyCode::NumpadEnter => Key::Enter,
        KeyCode::Backspace => Key::Backspace { step },
        KeyCode::Delete => Key::Delete { step },
        KeyCode::ArrowLeft => Key::Left { step, anchor },
        KeyCode::ArrowRight => Key::Right { step, anchor },
        KeyCode::Home => Key::Home { anchor },
        KeyCode::End => Key::End { anchor },
        KeyCode::PageUp => Key::PageUp,
        KeyCode::PageDown => Key::PageDown,
        KeyCode::KeyA if modifiers.is_shortcut() => Key::SelectAll,
        KeyCode::KeyC if modifiers.is_shortcut() => Key::Copy,
        KeyCode::KeyX if modifiers.is_shortcut() => Key::Cut,
        _ => return None,
    })
}

pub(crate) fn pastes(code: KeyCode, modifiers: KeyModifiers) -> bool {
    modifiers.is_shortcut() && code == KeyCode::KeyV
}

pub(crate) fn clipboard_text(clipboard: Option<&mut Clipboard>) -> Option<String> {
    match clipboard?.fetch_text() {
        ClipboardRead::Ready(Ok(text)) => Some(text),
        _ => None,
    }
}

pub(crate) fn copy_text(clipboard: Option<&mut Clipboard>, text: &str) {
    if let Some(clipboard) = clipboard {
        let _ = clipboard.set_text(text);
    }
}

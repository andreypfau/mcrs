use bevy::clipboard::{Clipboard, ClipboardRead};
use bevy::ecs::system::SystemParam;
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::math::DVec2;
use bevy::prelude::*;
use bevy::time::Real;
use bevy::window::PrimaryWindow;
use mcrs_minecraft_client_widget::{
    GuiKey, GuiPoint, KeyModifiers, PointerButton, PointerPress, WidgetEvent, Widgets,
};

use crate::menu::Menu;
use crate::menu_event::MenuEvent;
use crate::scene::{self, GuiAtlas, GuiConfig, GuiScene};

const BUTTONS: [(MouseButton, PointerButton); 3] = [
    (MouseButton::Left, PointerButton::Left),
    (MouseButton::Right, PointerButton::Right),
    (MouseButton::Middle, PointerButton::Middle),
];

const CAPTURE_EXEMPT: [KeyCode; 3] = [KeyCode::Escape, KeyCode::F2, KeyCode::F11];

pub(crate) fn menu_clock(time: &Time<Real>) -> u64 {
    u64::try_from(time.elapsed().as_millis()).unwrap_or(u64::MAX)
}

pub(crate) fn text_width(atlas: Option<&GuiAtlas>) -> impl Fn(&str) -> i32 + '_ {
    move |text| atlas.map_or(0, |atlas| atlas.0.font.str_width(text))
}

#[derive(SystemParam)]
pub(crate) struct Pointer<'w, 's> {
    window: Single<'w, 's, &'static Window, With<PrimaryWindow>>,
    config: Res<'w, GuiConfig>,
    scene: Res<'w, GuiScene>,
    buttons: Res<'w, ButtonInput<MouseButton>>,
    scroll: Res<'w, AccumulatedMouseScroll>,
    last: Local<'s, Option<DVec2>>,
}

impl Pointer<'_, '_> {
    fn is_idle(&mut self, at: Option<DVec2>, mouse: GuiPoint) -> bool {
        let moved = std::mem::replace(&mut *self.last, at) != at
            || at.is_some_and(|at| scene::gui_point(at) != mouse);
        let acted = BUTTONS.iter().any(|&(button, _)| {
            self.buttons.just_pressed(button) || self.buttons.just_released(button)
        });
        !moved && !acted && self.scroll.delta.y == 0.0
    }

    fn position(&self) -> Option<DVec2> {
        scene::gui_pointer(&self.window, &self.config, self.scene.size.scale)
    }
}

pub(crate) fn menu_pointer(
    mut menu: ResMut<Menu>,
    mut pointer: Pointer,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time<Real>>,
    atlas: Option<Res<GuiAtlas>>,
    mut out: MessageWriter<MenuEvent>,
    mut events: Local<Vec<WidgetEvent>>,
) {
    let at = pointer.position();
    if pointer.is_idle(at, menu.widgets.input().mouse()) {
        return;
    }
    let now_ms = menu_clock(&time);
    let modifiers = modifiers(&keys);
    let width = text_width(atlas.as_deref());
    let widgets = &mut menu.widgets;
    let buttons = &*pointer.buttons;
    if let Some(at) = at {
        widgets.mouse_move(scene::gui_point(at));
        if buttons.pressed(MouseButton::Left) && !buttons.just_pressed(MouseButton::Left) {
            widgets.mouse_drag(at.x, now_ms, &width, &mut events);
        }
        for (button, pressed) in BUTTONS {
            if buttons.just_pressed(button) {
                widgets.mouse_press(
                    PointerPress {
                        x: at.x,
                        y: at.y,
                        button: pressed,
                        modifiers,
                        now_ms,
                    },
                    &width,
                    &mut events,
                );
            }
        }
        let down = f64::from(pointer.scroll.delta.y);
        if down != 0.0 {
            widgets.mouse_wheel(at.x, at.y, down, &mut events);
        }
    }
    for (button, released) in BUTTONS {
        if buttons.just_released(button) {
            widgets.mouse_release(released);
        }
    }
    out.write_batch(events.drain(..).map(MenuEvent::Widget));
}

#[derive(SystemParam)]
pub(crate) struct MenuKeys<'w, 's> {
    keyboard: MessageReader<'w, 's, KeyboardInput>,
    keys: ResMut<'w, ButtonInput<KeyCode>>,
    clipboard: Option<ResMut<'w, Clipboard>>,
}

pub(crate) fn menu_keys(
    mut menu: ResMut<Menu>,
    mut input: MenuKeys,
    time: Res<Time<Real>>,
    atlas: Option<Res<GuiAtlas>>,
    mut out: MessageWriter<MenuEvent>,
    mut events: Local<Vec<WidgetEvent>>,
) {
    if menu.is_added() {
        input.keyboard.clear();
        return;
    }
    let now_ms = menu_clock(&time);
    let modifiers = modifiers(&input.keys);
    let width = text_width(atlas.as_deref());
    for key in input.keyboard.read() {
        if key.state != ButtonState::Pressed {
            continue;
        }
        if menu.widgets.captures_keyboard() {
            capture_key(&mut input.keys, key.key_code);
        }
        if key.key_code == KeyCode::Escape {
            out.write(MenuEvent::CloseRequested);
            continue;
        }
        let widgets = &mut menu.widgets;
        if modifiers.is_shortcut()
            && let Some(shortcut) = Shortcut::of(&key.logical_key)
        {
            let read = || input.clipboard.as_deref_mut().and_then(clipboard_text);
            let copied = shortcut.apply(widgets, read, &width, &mut events);
            if let (Some(text), Some(clipboard)) = (copied, input.clipboard.as_deref_mut()) {
                let _ = clipboard.set_text(text);
            }
        } else if !typed(key, modifiers)
            .is_some_and(|text| widgets.type_text(text, &width, &mut events))
            && let Some(gui) = gui_key(key.key_code)
        {
            widgets.key_press(gui, modifiers, now_ms, &width, &mut events);
            if events.is_empty() && matches!(gui, GuiKey::Enter | GuiKey::NumpadEnter) {
                out.write(MenuEvent::Confirmed);
            }
        }
        out.write_batch(events.drain(..).map(MenuEvent::Widget));
    }
}

pub(crate) fn capture_key(keys: &mut ButtonInput<KeyCode>, code: KeyCode) {
    if !CAPTURE_EXEMPT.contains(&code) {
        keys.clear_just_pressed(code);
    }
}

fn typed(key: &KeyboardInput, modifiers: KeyModifiers) -> Option<&str> {
    key.text
        .as_deref()
        .filter(|text| !modifiers.control && !text.chars().any(char::is_control))
}

#[derive(Clone, Copy)]
enum Shortcut {
    SelectAll,
    Copy,
    Cut,
    Paste,
}

impl Shortcut {
    fn of(key: &Key) -> Option<Self> {
        let Key::Character(text) = key else {
            return None;
        };
        [
            ("a", Self::SelectAll),
            ("c", Self::Copy),
            ("x", Self::Cut),
            ("v", Self::Paste),
        ]
        .into_iter()
        .find_map(|(letter, shortcut)| text.eq_ignore_ascii_case(letter).then_some(shortcut))
    }

    fn apply(
        self,
        widgets: &mut Widgets,
        clipboard_text: impl FnOnce() -> Option<String>,
        width: &impl Fn(&str) -> i32,
        events: &mut Vec<WidgetEvent>,
    ) -> Option<String> {
        match self {
            Self::SelectAll => {
                widgets.select_all();
                None
            }
            Self::Copy => widgets.copy().map(str::to_owned),
            Self::Cut => widgets.cut(width, events),
            Self::Paste => {
                if let Some(text) = clipboard_text() {
                    widgets.paste(&text, width, events);
                }
                None
            }
        }
    }
}

fn clipboard_text(clipboard: &mut Clipboard) -> Option<String> {
    match clipboard.fetch_text() {
        ClipboardRead::Ready(Ok(text)) => Some(text),
        _ => None,
    }
}

pub(crate) fn menu_deferred(
    mut menu: ResMut<Menu>,
    time: Res<Time<Real>>,
    mut out: MessageWriter<MenuEvent>,
    mut events: Local<Vec<WidgetEvent>>,
) {
    menu.bypass_change_detection()
        .widgets
        .apply_deferred(menu_clock(&time), &mut events);
    if !events.is_empty() {
        menu.set_changed();
    }
    out.write_batch(events.drain(..).map(MenuEvent::Widget));
}

pub(crate) fn modifiers(keys: &ButtonInput<KeyCode>) -> KeyModifiers {
    let mut modifiers = KeyModifiers::NONE;
    modifiers.shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    modifiers.control = keys.any_pressed([
        KeyCode::ControlLeft,
        KeyCode::ControlRight,
        KeyCode::SuperLeft,
        KeyCode::SuperRight,
    ]);
    modifiers.alt = keys.any_pressed([KeyCode::AltLeft, KeyCode::AltRight]);
    modifiers
}

pub(crate) fn gui_key(code: KeyCode) -> Option<GuiKey> {
    Some(match code {
        KeyCode::Tab => GuiKey::Tab,
        KeyCode::Enter => GuiKey::Enter,
        KeyCode::Space => GuiKey::Space,
        KeyCode::NumpadEnter => GuiKey::NumpadEnter,
        KeyCode::ArrowUp => GuiKey::Up,
        KeyCode::ArrowDown => GuiKey::Down,
        KeyCode::ArrowLeft => GuiKey::Left,
        KeyCode::ArrowRight => GuiKey::Right,
        KeyCode::Backspace => GuiKey::Backspace,
        KeyCode::Delete => GuiKey::Delete,
        KeyCode::Home => GuiKey::Home,
        KeyCode::End => GuiKey::End,
        KeyCode::Digit0 | KeyCode::Numpad0 => GuiKey::Digit(0),
        KeyCode::Digit1 | KeyCode::Numpad1 => GuiKey::Digit(1),
        KeyCode::Digit2 | KeyCode::Numpad2 => GuiKey::Digit(2),
        KeyCode::Digit3 | KeyCode::Numpad3 => GuiKey::Digit(3),
        KeyCode::Digit4 | KeyCode::Numpad4 => GuiKey::Digit(4),
        KeyCode::Digit5 | KeyCode::Numpad5 => GuiKey::Digit(5),
        KeyCode::Digit6 | KeyCode::Numpad6 => GuiKey::Digit(6),
        KeyCode::Digit7 | KeyCode::Numpad7 => GuiKey::Digit(7),
        KeyCode::Digit8 | KeyCode::Numpad8 => GuiKey::Digit(8),
        KeyCode::Digit9 | KeyCode::Numpad9 => GuiKey::Digit(9),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use mcrs_minecraft_client_widget::{
        EditBox, GuiExtent, GuiInput, GuiPoint, GuiRect, Widget, WidgetId, WidgetKind, WidgetValue,
    };

    use super::*;

    #[test]
    fn a_shortcut_reads_or_returns_the_clipboard_text_of_a_selected_value() {
        const EDIT: WidgetId = WidgetId(1);
        let width = |text: &str| text.len() as i32 * 6;
        let cases = [
            (Shortcut::Copy, Some("ab"), "ab"),
            (Shortcut::Cut, Some("ab"), ""),
            (Shortcut::Paste, None, "xy"),
        ];
        for (shortcut, copied, text) in cases {
            let mut widgets =
                Widgets::new(GuiExtent::new(320, 240), GuiInput::new(GuiPoint::new(0, 0)));
            let mut edit = EditBox::default();
            edit.set("ab");
            let rect = GuiRect::new(10, 10, 150, 20);
            let widget = Widget::new(EDIT, rect, String::new(), WidgetKind::EditBox(edit));
            widgets.push(widget).unwrap();
            widgets.focus_initial(Some(EDIT), 0, &mut Vec::new());
            widgets.select_all();
            let read = || Some("xy".to_owned());
            let returned = shortcut.apply(&mut widgets, read, &width, &mut Vec::new());
            let value = match widgets.value(EDIT) {
                Some(WidgetValue::Text(text)) => text,
                _ => "",
            };
            assert_eq!((returned.as_deref(), value), (copied, text));
        }
    }
}

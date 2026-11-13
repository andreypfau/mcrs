use mcrs_minecraft_client_widget::{
    GuiExtent, GuiInput, GuiKey, GuiPoint, GuiRect, KeyModifiers, PointerButton, PointerPress,
    Widget, WidgetEvent, WidgetId, WidgetKind, Widgets,
};

pub const SCREEN: GuiExtent = GuiExtent::new(427, 240);
pub const AWAY: GuiPoint = GuiPoint::new(-100, -100);

pub fn widget(id: u32, rect: GuiRect, kind: WidgetKind) -> Widget {
    Widget::new(WidgetId(id), rect, String::new(), kind)
}

pub fn button(id: u32, x: i32, y: i32, width: i32, height: i32) -> Widget {
    widget(id, GuiRect::new(x, y, width, height), WidgetKind::Button)
}

pub fn screen_of(widgets: impl IntoIterator<Item = Widget>) -> Widgets {
    screen_with_input(GuiInput::new(AWAY), widgets)
}

pub fn screen_with_input(input: GuiInput, widgets: impl IntoIterator<Item = Widget>) -> Widgets {
    let mut screen = Widgets::new(SCREEN, input);
    for widget in widgets {
        assert!(
            screen.push(widget).is_ok(),
            "the ids of a fixture are distinct"
        );
    }
    screen
}

/// A font whose glyphs are each 6 GUI units wide.
pub fn mono(text: &str) -> i32 {
    i32::try_from(text.chars().count() * 6).unwrap_or(i32::MAX)
}

pub fn press(screen: &mut Widgets, x: f64, y: f64, button: PointerButton) -> Vec<WidgetEvent> {
    let mut events = Vec::new();
    screen.mouse_press(
        PointerPress {
            x,
            y,
            button,
            modifiers: KeyModifiers::NONE,
            now_ms: 0,
        },
        &mono,
        &mut events,
    );
    events
}

pub fn key(screen: &mut Widgets, key: GuiKey, modifiers: KeyModifiers) -> Vec<WidgetEvent> {
    let mut events = Vec::new();
    screen.key_press(key, modifiers, 0, &mono, &mut events);
    events
}

pub fn focus_of(id: u32) -> WidgetEvent {
    WidgetEvent::FocusChanged {
        focused: Some(WidgetId(id)),
    }
}

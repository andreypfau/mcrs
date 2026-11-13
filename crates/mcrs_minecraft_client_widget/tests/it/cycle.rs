use mcrs_minecraft_client_widget::{
    CycleButton, GuiKey, GuiRect, KeyModifiers, PointerButton, PointerPress, WidgetEvent, WidgetId,
    WidgetKind, Widgets,
};

use crate::screen::{mono, screen_of, widget};

enum Input {
    Press(PointerButton, KeyModifiers),
    Key(GuiKey, KeyModifiers),
    Wheel(f64),
}

fn index(screen: &Widgets) -> Option<usize> {
    match screen.get(WidgetId(1)).map(|widget| &widget.kind) {
        Some(WidgetKind::CycleButton(cycle)) => Some(cycle.index()),
        _ => None,
    }
}

#[test]
fn an_activation_selects_the_next_value_shift_the_previous_and_the_wheel_up_the_previous() {
    use Input::{Key, Press, Wheel};
    let (none, shift) = (KeyModifiers::NONE, KeyModifiers::SHIFT);
    let steps = [
        (Press(PointerButton::Left, none), 1, true),
        (Press(PointerButton::Left, none), 2, true),
        (Press(PointerButton::Left, none), 0, true),
        (Press(PointerButton::Left, shift), 2, true),
        (Press(PointerButton::Right, none), 2, false),
        (Key(GuiKey::Enter, none), 0, true),
        (Key(GuiKey::Space, shift), 2, true),
        (Wheel(1.0), 1, true),
        (Wheel(-2.0), 2, true),
        (Wheel(-1.0), 0, true),
        (Wheel(0.0), 0, false),
    ];
    let cycle = WidgetKind::CycleButton(CycleButton::new(3, 0).unwrap());
    let mut screen = screen_of([widget(1, GuiRect::new(0, 0, 150, 20), cycle)]);
    let changed = WidgetEvent::ValueChanged {
        widget: WidgetId(1),
    };
    for (step, (input, expected, changes)) in steps.into_iter().enumerate() {
        let mut events = Vec::new();
        match input {
            Press(button, modifiers) => {
                screen.mouse_press(
                    PointerPress {
                        x: 10.0,
                        y: 10.0,
                        button,
                        modifiers,
                        now_ms: 0,
                    },
                    &mono,
                    &mut events,
                );
            }
            Key(key, modifiers) => screen.key_press(key, modifiers, 0, &mono, &mut events),
            Wheel(down) => screen.mouse_wheel(10.0, 10.0, down, &mut events),
        }
        assert_eq!(index(&screen), Some(expected), "step {step}");
        assert_eq!(
            events.contains(&changed),
            changes,
            "step {step}: {events:?}"
        );
    }
}

#[test]
fn a_boolean_cycle_button_starts_with_true() {
    let mut screen = screen_of([widget(
        1,
        GuiRect::new(0, 0, 150, 20),
        WidgetKind::CycleButton(CycleButton::boolean(true)),
    )]);
    assert_eq!(index(&screen), Some(0));
    screen.mouse_wheel(10.0, 10.0, -1.0, &mut Vec::new());
    let cycle = screen
        .get(WidgetId(1))
        .and_then(|widget| match &widget.kind {
            WidgetKind::CycleButton(cycle) => Some(cycle),
            _ => None,
        });
    assert_eq!(cycle, Some(&CycleButton::boolean(false)));
}

use mcrs_minecraft_client_widget::{
    GuiKey, GuiPoint, GuiRect, KeyModifiers, PointerButton, PointerPress, PointerShape, Slider,
    WidgetEvent, WidgetId, WidgetKind, WidgetSprite, Widgets,
};

use crate::screen::{button, mono, screen_of, widget};

const BAR: GuiRect = GuiRect::new(0, 30, 208, 20);

fn slider_screen(slider: Slider) -> Widgets {
    screen_of([
        button(1, 0, 0, 208, 20),
        widget(2, BAR, WidgetKind::Slider(slider)),
        button(3, 220, 30, 100, 20),
        button(4, 0, 60, 208, 20),
    ])
}

fn slider(screen: &Widgets) -> Option<Slider> {
    match screen.get(WidgetId(2)).map(|widget| &widget.kind) {
        Some(WidgetKind::Slider(slider)) => Some(*slider),
        _ => None,
    }
}

fn sprite(screen: &Widgets) -> Option<WidgetSprite> {
    screen
        .get(WidgetId(2))
        .and_then(|widget| screen.sprite(widget))
}

fn handle_x(screen: &Widgets) -> Option<i32> {
    match sprite(screen) {
        Some(WidgetSprite::Slider { handle_rect, .. }) => Some(handle_rect.x),
        _ => None,
    }
}

fn press(screen: &mut Widgets, x: f64, now_ms: u64) -> Vec<WidgetEvent> {
    let mut events = Vec::new();
    let modifiers = KeyModifiers::NONE;
    screen.mouse_press(
        PointerPress {
            x,
            y: 40.0,
            button: PointerButton::Left,
            modifiers,
            now_ms,
        },
        &mono,
        &mut events,
    );
    events
}

fn drag(screen: &mut Widgets, x: f64, now_ms: u64) -> Vec<WidgetEvent> {
    let mut events = Vec::new();
    screen.mouse_drag(x, now_ms, &mono, &mut events);
    events
}

const CHANGED: WidgetEvent = WidgetEvent::ValueChanged {
    widget: WidgetId(2),
};
const PENDING: WidgetEvent = WidgetEvent::ValuePending {
    widget: WidgetId(2),
};

#[test]
fn an_option_slider_gives_each_value_an_equal_part_of_the_bar() {
    let range = || Slider::range(30, 110, 70).unwrap();
    let list = || Slider::list(5, 2).unwrap();
    let cases = [
        (range(), 0.0, Some(30), None),
        (range(), 4.0, Some(30), None),
        (range(), 102.0, Some(69), None),
        (range(), 103.0, Some(70), None),
        (range(), 105.0, Some(70), None),
        (range(), 106.0, Some(71), None),
        (range(), 204.0, Some(110), None),
        (range(), 207.5, Some(110), None),
        (list(), 83.0, None, Some(1)),
        (list(), 105.0, None, Some(2)),
        (list(), 204.0, None, Some(4)),
    ];
    for (start, x, value, index) in cases {
        let mut screen = slider_screen(start);
        press(&mut screen, x, 0);
        let slider = slider(&screen).unwrap();
        assert_eq!(
            (slider.value(), slider.index()),
            (value, index),
            "press at {x}"
        );
    }
}

#[test]
fn the_handle_of_a_value_sits_at_the_position_of_that_value() {
    let cases = [
        (Slider::range(30, 110, 70), 0.5, 100),
        (Slider::range(30, 110, 30), 0.0, 0),
        (Slider::range(30, 110, 110), 1.0, 200),
        (Slider::list(5, 0), 0.0, 0),
        (Slider::list(5, 4), 1.0, 200),
        (Slider::list(5, 1), 0.25, 50),
        (Some(Slider::continuous(-3.0)), 0.0, 0),
    ];
    for (start, position, x) in cases {
        let screen = slider_screen(start.unwrap());
        assert_eq!(
            slider(&screen).map(|slider| slider.position()),
            Some(position)
        );
        assert_eq!(handle_x(&screen), Some(x), "{start:?}");
    }
    assert_eq!(Slider::range(5, 5, 5), None);
    assert_eq!(Slider::list(1, 0), None);
    assert_eq!(Slider::list(3, 3), None);
}

#[test]
fn a_press_at_the_centre_of_the_handle_keeps_the_position_and_a_drag_past_the_end_applies_once() {
    let mut screen = slider_screen(Slider::range(30, 110, 70).unwrap());
    assert!(
        drag(&mut screen, 150.0, 0).is_empty(),
        "no drag before a press"
    );
    let focus = WidgetEvent::FocusChanged {
        focused: Some(WidgetId(2)),
    };
    assert_eq!(press(&mut screen, 104.0, 0), [focus]);
    assert_eq!(drag(&mut screen, 300.0, 0), [CHANGED]);
    assert!(drag(&mut screen, 400.0, 0).is_empty());
    assert_eq!(drag(&mut screen, -50.0, 0), [CHANGED]);
    assert_eq!(slider(&screen).and_then(|slider| slider.value()), Some(30));
    screen.mouse_release(PointerButton::Left);
    assert!(
        drag(&mut screen, 150.0, 0).is_empty(),
        "no drag after the release"
    );
}

#[test]
fn the_handle_of_an_integer_slider_jumps_to_the_centre_of_its_part_at_the_release() {
    let mut screen = slider_screen(Slider::range(30, 110, 70).unwrap());
    press(&mut screen, 110.0, 0);
    assert_eq!(handle_x(&screen), Some(106));
    screen.mouse_release(PointerButton::Right);
    assert_eq!(
        handle_x(&screen),
        Some(106),
        "only the left button ends the drag"
    );
    screen.mouse_release(PointerButton::Left);
    assert_eq!(slider(&screen).and_then(|slider| slider.value()), Some(72));
    assert_eq!(handle_x(&screen), Some(104));
}

#[test]
fn a_deferred_slider_stores_its_value_in_the_first_frame_600_ms_after_the_last_change() {
    let mut screen = slider_screen(Slider::range(30, 110, 70).unwrap().deferred());
    assert_eq!(press(&mut screen, 110.0, 1000)[..1], [PENDING]);
    screen.mouse_release(PointerButton::Left);
    assert_eq!(
        handle_x(&screen),
        Some(106),
        "the release does not move the handle"
    );
    let frames = [
        (1000, false),
        (1599, false),
        (1600, true),
        (1601, false),
        (5000, false),
    ];
    for (now_ms, applies) in frames {
        let mut events = Vec::new();
        screen.apply_deferred(now_ms, &mut events);
        assert_eq!(events == [CHANGED], applies, "frame at {now_ms} ms");
    }
    assert_eq!(handle_x(&screen), Some(104));
    press(&mut screen, 150.0, 6000);
    assert_eq!(drag(&mut screen, 160.0, 6500), [PENDING]);
    let mut events = Vec::new();
    screen.apply_deferred(7099, &mut events);
    assert!(events.is_empty(), "the delay counts from the last change");
}

#[test]
fn change_mode_starts_after_tab_and_the_mouse_and_takes_left_and_right_until_an_activation_key() {
    let (none, shift) = (KeyModifiers::NONE, KeyModifiers::SHIFT);
    let steps = [
        (GuiKey::Tab, none, 1, 5, false, false),
        (GuiKey::Tab, none, 2, 5, false, true),
        (GuiKey::Right, none, 2, 6, false, true),
        (GuiKey::Left, none, 2, 5, false, true),
        (GuiKey::Enter, none, 2, 5, true, false),
        (GuiKey::Right, none, 3, 5, false, false),
        (GuiKey::Left, none, 2, 5, true, false),
        (GuiKey::Space, none, 2, 5, false, true),
        (GuiKey::Right, none, 2, 6, false, true),
        (GuiKey::Down, none, 4, 6, false, false),
        (GuiKey::Up, none, 2, 6, true, false),
        (GuiKey::Tab, none, 3, 6, false, false),
        (GuiKey::Tab, shift, 2, 6, false, true),
        (GuiKey::Left, none, 2, 5, false, true),
    ];
    let mut screen = slider_screen(Slider::range(0, 10, 5).unwrap());
    for (step, (key, modifiers, focused, value, bar, handle)) in steps.into_iter().enumerate() {
        screen.key_press(key, modifiers, 0, &mono, &mut Vec::new());
        assert_eq!(screen.focused(), Some(WidgetId(focused)), "step {step}");
        assert_eq!(
            slider(&screen).and_then(|slider| slider.value()),
            Some(value),
            "step {step}"
        );
        assert!(
            matches!(
                sprite(&screen),
                Some(WidgetSprite::Slider { bar_highlighted, handle_highlighted, .. })
                    if bar_highlighted == bar && handle_highlighted == handle
            ),
            "step {step}: {:?}",
            sprite(&screen)
        );
    }
}

#[test]
fn a_key_step_moves_one_value_or_one_unit_of_the_bar_and_stops_at_the_ends() {
    let cases = [
        (Slider::range(0, 10, 10), 204.0, GuiKey::Right, 200, false),
        (Slider::range(0, 10, 0), 4.0, GuiKey::Left, 0, false),
        (
            Slider::range(0, 10, 9),
            4.0 + 200.0 * 9.5 / 11.0,
            GuiKey::Right,
            200,
            true,
        ),
        (Slider::list(3, 2), 204.0, GuiKey::Right, 200, false),
        (Slider::list(3, 0), 4.0, GuiKey::Right, 100, true),
        (
            Some(Slider::continuous(0.5)),
            104.0,
            GuiKey::Right,
            101,
            true,
        ),
        (Some(Slider::continuous(0.5)), 104.0, GuiKey::Left, 99, true),
    ];
    for (start, centre, key, x, changes) in cases {
        let mut screen = slider_screen(start.unwrap());
        assert_eq!(
            press(&mut screen, centre, 0).len(),
            1,
            "{start:?}: only the focus"
        );
        screen.mouse_release(PointerButton::Left);
        let mut events = Vec::new();
        screen.key_press(key, KeyModifiers::NONE, 0, &mono, &mut events);
        assert_eq!(
            events == [CHANGED],
            changes,
            "{start:?} {key:?}: {events:?}"
        );
        assert_eq!(handle_x(&screen), Some(x), "{start:?} {key:?}");
        assert_eq!(screen.focused(), Some(WidgetId(2)));
    }
}

#[test]
fn a_dragged_slider_shows_the_resize_pointer() {
    let mut screen = slider_screen(Slider::continuous(0.5));
    screen.mouse_move(GuiPoint::new(50, 40));
    assert_eq!(screen.pointer_shape(), PointerShape::Hand);
    press(&mut screen, 50.0, 0);
    assert_eq!(screen.pointer_shape(), PointerShape::ResizeHorizontal);
    screen.mouse_release(PointerButton::Left);
    assert_eq!(screen.pointer_shape(), PointerShape::Hand);
}

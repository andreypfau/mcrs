use mcrs_minecraft_client_widget::{
    ButtonSprite, GuiInput, GuiKey, GuiPoint, GuiRect, InputDevice, KeyModifiers, PointerButton,
    PointerShape, WidgetEvent, WidgetId, WidgetKind, WidgetSprite,
};

use crate::screen::{button, focus_of, key, press, screen_of, screen_with_input, widget};

#[test]
fn a_move_beyond_seven_units_from_the_anchor_gives_the_device_to_the_mouse() {
    let cases = [
        (7, 0, InputDevice::Tab),
        (4, 5, InputDevice::Tab),
        (5, 5, InputDevice::Mouse),
        (0, -8, InputDevice::Mouse),
    ];
    for (across, down, device) in cases {
        let mut screen = screen_with_input(GuiInput::new(GuiPoint::new(100, 100)), []);
        key(&mut screen, GuiKey::Tab, KeyModifiers::NONE);
        screen.mouse_move(GuiPoint::new(100 + across, 100 + down));
        assert_eq!(
            screen.input().device(),
            device,
            "move by ({across}, {down})"
        );
    }
}

#[test]
fn the_anchor_is_the_mouse_position_at_the_last_change_of_device() {
    let mut screen = screen_with_input(GuiInput::new(GuiPoint::new(100, 100)), []);
    key(&mut screen, GuiKey::Tab, KeyModifiers::NONE);
    screen.mouse_move(GuiPoint::new(105, 100));
    key(&mut screen, GuiKey::Down, KeyModifiers::NONE);
    screen.mouse_move(GuiPoint::new(110, 100));
    assert_eq!(screen.input().device(), InputDevice::Arrow);
    screen.mouse_move(GuiPoint::new(113, 100));
    assert_eq!(screen.input().device(), InputDevice::Mouse);
}

#[test]
fn a_press_goes_to_the_first_active_visible_widget_under_the_event_position() {
    let mut inactive = button(1, 0, 0, 100, 20);
    inactive.active = false;
    let mut hidden = button(3, 0, 30, 100, 20);
    hidden.visible = false;
    let widgets = [
        inactive,
        button(2, 0, 0, 100, 20),
        hidden,
        button(4, 0, 30, 100, 20),
        button(5, 0, 60, 100, 20),
        button(6, 0, 60, 100, 20),
        button(7, 200, 0, 0, 20),
    ];
    let cases = [
        (10.0, 10.0, PointerButton::Left, Some(2)),
        (99.5, 19.5, PointerButton::Left, Some(2)),
        (100.0, 10.0, PointerButton::Left, None),
        (10.0, 20.0, PointerButton::Left, None),
        (10.0, 35.0, PointerButton::Left, Some(4)),
        (10.0, 65.0, PointerButton::Left, Some(5)),
        (200.0, 10.0, PointerButton::Left, None),
        (10.0, 10.0, PointerButton::Right, None),
        (10.0, 10.0, PointerButton::Middle, None),
    ];
    for (x, y, pointer, target) in cases {
        let mut screen = screen_of(widgets.clone());
        let events = press(&mut screen, x, y, pointer);
        let expected: Vec<WidgetEvent> = target
            .map(|id| {
                vec![
                    WidgetEvent::Activated {
                        widget: WidgetId(id),
                        modifiers: KeyModifiers::NONE,
                    },
                    focus_of(id),
                ]
            })
            .unwrap_or_default();
        assert_eq!(events, expected, "press at ({x}, {y}) with {pointer:?}");
        assert_eq!(
            screen.focused(),
            target.map(WidgetId),
            "focus after ({x}, {y})"
        );
        assert_eq!(screen.input().device(), InputDevice::Mouse);
    }
}

#[test]
fn a_checkbox_toggles_on_a_press_and_on_an_activation_key_while_active() {
    let rect = GuiRect::new(0, 0, 60, 17);
    let mut screen = screen_of([widget(1, rect, WidgetKind::Checkbox { selected: false })]);
    let selected = |screen: &mcrs_minecraft_client_widget::Widgets| {
        matches!(
            screen.get(WidgetId(1)).unwrap().kind,
            WidgetKind::Checkbox { selected: true }
        )
    };
    let changed = WidgetEvent::ValueChanged {
        widget: WidgetId(1),
    };
    assert_eq!(
        press(&mut screen, 5.0, 5.0, PointerButton::Left),
        [changed, focus_of(1)]
    );
    assert!(selected(&screen), "the press selects the checkbox");
    assert_eq!(
        key(&mut screen, GuiKey::Space, KeyModifiers::NONE),
        [changed]
    );
    assert!(!selected(&screen), "Space clears the checkbox again");
    screen.get_mut(WidgetId(1)).unwrap().active = false;
    assert!(key(&mut screen, GuiKey::Enter, KeyModifiers::NONE).is_empty());
    assert!(
        !selected(&screen),
        "an inactive checkbox takes no activation key"
    );
}

#[test]
fn hover_highlight_and_pointer_follow_the_last_input_device() {
    let mut disabled = button(2, 0, 30, 100, 20);
    disabled.active = false;
    let mut screen = screen_of([button(1, 0, 0, 100, 20), disabled]);
    let sprite = |screen: &mcrs_minecraft_client_widget::Widgets, id| {
        screen.sprite(screen.get(WidgetId(id)).unwrap())
    };
    assert_eq!(
        sprite(&screen, 1),
        Some(WidgetSprite::Button(ButtonSprite::Normal))
    );
    screen.mouse_move(GuiPoint::new(10, 10));
    assert_eq!(screen.pointer_shape(), PointerShape::Hand);
    assert_eq!(
        sprite(&screen, 1),
        Some(WidgetSprite::Button(ButtonSprite::Highlighted))
    );
    screen.mouse_move(GuiPoint::new(10, 40));
    assert_eq!(screen.pointer_shape(), PointerShape::NotAllowed);
    assert!(screen.is_hovered(screen.get(WidgetId(2)).unwrap()));
    assert_eq!(
        sprite(&screen, 2),
        Some(WidgetSprite::Button(ButtonSprite::Disabled))
    );
    key(&mut screen, GuiKey::Tab, KeyModifiers::NONE);
    assert!(!screen.is_hovered(screen.get(WidgetId(2)).unwrap()));
    assert_eq!(screen.pointer_shape(), PointerShape::Normal);
    assert_eq!(
        sprite(&screen, 1),
        Some(WidgetSprite::Button(ButtonSprite::Highlighted))
    );
}

#[test]
fn an_activation_key_runs_the_focused_button_with_its_modifiers() {
    let mut screen = screen_of([button(1, 0, 0, 100, 20)]);
    key(&mut screen, GuiKey::Tab, KeyModifiers::NONE);
    assert_eq!(
        key(&mut screen, GuiKey::NumpadEnter, KeyModifiers::SHIFT),
        [WidgetEvent::Activated {
            widget: WidgetId(1),
            modifiers: KeyModifiers::SHIFT,
        }]
    );
    assert!(key(&mut screen, GuiKey::Left, KeyModifiers::NONE).is_empty());
}

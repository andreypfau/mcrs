use mcrs_minecraft_client_widget::{
    GuiKey, GuiRect, KeyModifiers, Overflow, Widget, WidgetEvent, WidgetId, WidgetIdTaken,
    WidgetKind, Widgets,
};

use crate::screen::{button, key, screen_of, widget};

fn grouped(id: u32, group: i32) -> Widget {
    let mut widget = button(id, 0, 0, 100, 20);
    widget.tab_group = group;
    widget
}

fn tab_screen() -> Widgets {
    let mut inactive = grouped(4, 0);
    inactive.active = false;
    let mut hidden = grouped(5, 0);
    hidden.visible = false;
    let text = widget(
        6,
        GuiRect::new(0, 200, 50, 9),
        WidgetKind::Text {
            max_width: None,
            overflow: Overflow::Cut,
        },
    );
    screen_of([
        grouped(1, 0),
        grouped(2, 1),
        grouped(3, 0),
        inactive,
        hidden,
        text,
    ])
}

fn focus_sequence(
    screen: &mut Widgets,
    pressed: GuiKey,
    modifiers: KeyModifiers,
    count: usize,
) -> Vec<u32> {
    (0..count)
        .map(|_| {
            key(screen, pressed, modifiers);
            screen.focused().map_or(0, |id| id.0)
        })
        .collect()
}

#[test]
fn tab_walks_the_focusable_widgets_by_group_then_addition_and_wraps() {
    let cases = [
        (KeyModifiers::NONE, vec![1, 3, 2, 1, 3]),
        (KeyModifiers::SHIFT, vec![2, 3, 1, 2, 3]),
    ];
    for (modifiers, expected) in cases {
        let mut screen = tab_screen();
        assert_eq!(
            focus_sequence(&mut screen, GuiKey::Tab, modifiers, 5),
            expected,
            "{modifiers:?}"
        );
    }
}

#[test]
fn tab_keeps_a_lone_focusable_widget_and_clears_when_none_is_left() {
    let mut screen = screen_of([button(1, 0, 0, 100, 20)]);
    key(&mut screen, GuiKey::Tab, KeyModifiers::NONE);
    assert!(key(&mut screen, GuiKey::Tab, KeyModifiers::NONE).is_empty());
    assert_eq!(screen.focused(), Some(WidgetId(1)));
    screen.get_mut(WidgetId(1)).unwrap().active = false;
    assert_eq!(
        key(&mut screen, GuiKey::Tab, KeyModifiers::NONE),
        [WidgetEvent::FocusChanged { focused: None }]
    );
}

fn arrow_screen() -> Widgets {
    screen_of([
        button(1, 0, 0, 98, 20),
        button(2, 106, 0, 98, 20),
        button(3, 0, 28, 204, 20),
        button(4, 0, 56, 98, 20),
        button(5, 106, 56, 98, 20),
        button(6, 300, 100, 50, 20),
    ])
}

#[test]
fn an_arrow_focuses_the_nearest_widget_on_its_side_or_the_nearest_point_after_it() {
    let cases = [
        (None, GuiKey::Down, Some(1)),
        (None, GuiKey::Up, Some(6)),
        (None, GuiKey::Right, Some(1)),
        (None, GuiKey::Left, Some(6)),
        (Some(1), GuiKey::Right, Some(2)),
        (Some(1), GuiKey::Down, Some(3)),
        (Some(2), GuiKey::Down, Some(3)),
        (Some(3), GuiKey::Down, Some(4)),
        (Some(5), GuiKey::Up, Some(3)),
        (Some(5), GuiKey::Down, Some(6)),
        (Some(6), GuiKey::Up, Some(5)),
        (Some(4), GuiKey::Left, Some(4)),
        (Some(2), GuiKey::Up, Some(2)),
    ];
    for (start, pressed, expected) in cases {
        let mut screen = arrow_screen();
        if let Some(start) = start {
            screen.focus_initial(Some(WidgetId(start)), 0, &mut Vec::new());
        }
        key(&mut screen, pressed, KeyModifiers::NONE);
        assert_eq!(
            screen.focused(),
            expected.map(WidgetId),
            "{pressed:?} from {start:?}"
        );
    }
}

#[test]
fn equal_near_edges_select_the_smaller_coordinate_across() {
    let cases = [
        (GuiKey::Down, (110, 140), (90, 140)),
        (GuiKey::Up, (110, 60), (90, 60)),
        (GuiKey::Right, (140, 110), (140, 90)),
        (GuiKey::Left, (60, 110), (60, 90)),
    ];
    for (pressed, (first_x, first_y), (second_x, second_y)) in cases {
        let mut screen = screen_of([
            button(1, 100, 100, 20, 20),
            button(2, first_x, first_y, 20, 20),
            button(3, second_x, second_y, 20, 20),
        ]);
        screen.focus_initial(Some(WidgetId(1)), 0, &mut Vec::new());
        key(&mut screen, pressed, KeyModifiers::NONE);
        assert_eq!(screen.focused(), Some(WidgetId(3)), "{pressed:?}");
    }
}

#[test]
fn the_initial_focus_takes_the_named_widget_else_the_first_for_tab_after_a_key() {
    let mut inactive = button(2, 0, 30, 100, 20);
    inactive.active = false;
    let widgets = [
        button(1, 0, 0, 100, 20),
        inactive,
        button(3, 0, 60, 100, 20),
    ];
    let mut keyboard = screen_of([]);
    key(&mut keyboard, GuiKey::Enter, KeyModifiers::NONE);
    let mut mouse = screen_of([]);
    crate::screen::press(
        &mut mouse,
        0.0,
        0.0,
        mcrs_minecraft_client_widget::PointerButton::Left,
    );
    let cases = [
        (keyboard.input(), None, Some(3), Some(3)),
        (mouse.input(), None, Some(3), Some(3)),
        (keyboard.input(), None, Some(2), Some(1)),
        (keyboard.input(), None, None, Some(1)),
        (mouse.input(), None, Some(2), None),
        (screen_of([]).input(), None, None, None),
        (keyboard.input(), Some(3), Some(3), Some(3)),
    ];
    for (input, before, initial, expected) in cases {
        let mut screen = crate::screen::screen_with_input(input, widgets.clone());
        screen.focus_initial(before.map(WidgetId), 0, &mut Vec::new());
        screen.focus_initial(initial.map(WidgetId), 0, &mut Vec::new());
        assert_eq!(
            screen.focused(),
            expected.map(WidgetId),
            "{:?} with {initial:?}",
            input.device()
        );
    }
}

#[test]
fn a_screen_refuses_a_taken_id_and_a_clear_drops_the_widgets_and_the_focus() {
    let mut screen = screen_of([button(1, 0, 0, 100, 20)]);
    screen.focus_initial(Some(WidgetId(1)), 0, &mut Vec::new());
    assert_eq!(
        screen.push(button(1, 0, 30, 100, 20)),
        Err(WidgetIdTaken(WidgetId(1)))
    );
    assert_eq!(screen.iter().count(), 1);
    screen.clear();
    assert_eq!((screen.iter().count(), screen.focused()), (0, None));
    assert_eq!(screen.push(button(1, 0, 30, 100, 20)), Ok(()));
}

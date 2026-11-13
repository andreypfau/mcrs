use mcrs_minecraft_client_widget::{
    AreaSlot, GuiKey, GuiRect, KeyModifiers, PointerButton, PointerPress, SelectionList, Widget,
    WidgetEvent, WidgetId, WidgetKind, WidgetList, Widgets,
};

use crate::screen::{button, key, mono, press, screen_of, widget};

const AREA: GuiRect = GuiRect::new(0, 33, 427, 168);
const LIST: WidgetId = WidgetId(2);
const TAB: (GuiKey, KeyModifiers) = (GuiKey::Tab, KeyModifiers::NONE);
const BACK: (GuiKey, KeyModifiers) = (GuiKey::Tab, KeyModifiers::SHIFT);
const UP: (GuiKey, KeyModifiers) = (GuiKey::Up, KeyModifiers::NONE);
const DOWN: (GuiKey, KeyModifiers) = (GuiKey::Down, KeyModifiers::NONE);
const LEFT: (GuiKey, KeyModifiers) = (GuiKey::Left, KeyModifiers::NONE);
const RIGHT: (GuiKey, KeyModifiers) = (GuiKey::Right, KeyModifiers::NONE);

/// A button above, a list of 10 rows of 36 and a button below.
fn selection_screen() -> Widgets {
    screen_of([
        button(1, 163, 5, 100, 20),
        widget(
            LIST.0,
            AREA,
            WidgetKind::SelectionList(SelectionList::new(10, 36)),
        ),
        button(3, 163, 210, 100, 20),
    ])
}

fn selected(screen: &Widgets) -> Option<usize> {
    match screen.get(LIST).map(|list| &list.kind) {
        Some(WidgetKind::SelectionList(list)) => list.selected(),
        _ => None,
    }
}

fn offset(screen: &Widgets) -> Option<f64> {
    screen.get(LIST).and_then(Widget::scroll_offset)
}

fn keys(screen: &mut Widgets, keys: &[(GuiKey, KeyModifiers)]) -> Vec<WidgetEvent> {
    keys.iter()
        .flat_map(|(pressed, modifiers)| key(screen, *pressed, *modifiers))
        .collect()
}

#[test]
fn press_selects_and_scrolls_to_the_row() {
    let cases = [
        ("visible row", 0.0, 200.0, 100.0, Some(1), 0.0),
        ("row under the bottom", 0.0, 200.0, 190.0, Some(4), 16.0),
        ("row under the top", -1.0, 200.0, 40.0, Some(0), 0.0),
        (
            "visible row at an offset",
            -1.0,
            200.0,
            196.0,
            Some(4),
            18.0,
        ),
        ("left of the rows", 0.0, 50.0, 100.0, None, 0.0),
    ];
    for (name, wheel, x, y, row, expected_offset) in cases {
        let mut screen = selection_screen();
        screen.mouse_wheel(200.0, 100.0, wheel, &mut Vec::new());
        press(&mut screen, x, y, PointerButton::Left);
        assert_eq!(
            (selected(&screen), offset(&screen)),
            (row, Some(expected_offset)),
            "{name}"
        );
    }
}

#[test]
fn keys_move_the_selection_and_scroll_to_it() {
    let cases: [(&str, &[_], _, _, _); 10] = [
        ("enter the list", &[TAB, TAB], Some(0), 2, 0.0),
        (
            "four rows down",
            &[TAB, TAB, DOWN, DOWN, DOWN, DOWN],
            Some(4),
            2,
            16.0,
        ),
        (
            "last row",
            &[
                TAB, TAB, DOWN, DOWN, DOWN, DOWN, DOWN, DOWN, DOWN, DOWN, DOWN,
            ],
            Some(9),
            2,
            196.0,
        ),
        (
            "down on the last row leaves the list",
            &[
                TAB, TAB, DOWN, DOWN, DOWN, DOWN, DOWN, DOWN, DOWN, DOWN, DOWN, DOWN,
            ],
            None,
            3,
            196.0,
        ),
        (
            "up on the first row leaves the list",
            &[TAB, TAB, UP],
            None,
            1,
            0.0,
        ),
        ("down into the list", &[TAB, DOWN], Some(0), 2, 0.0),
        (
            "right with nothing beside",
            &[TAB, TAB, RIGHT],
            None,
            2,
            0.0,
        ),
        (
            "down after right",
            &[TAB, TAB, RIGHT, DOWN],
            Some(0),
            2,
            0.0,
        ),
        (
            "shift tab enters at the last row",
            &[BACK, BACK],
            Some(9),
            2,
            196.0,
        ),
        (
            "tab away keeps the row",
            &[TAB, TAB, DOWN, TAB],
            Some(1),
            3,
            0.0,
        ),
    ];
    for (name, pressed, row, focused, expected_offset) in cases {
        let mut screen = selection_screen();
        keys(&mut screen, pressed);
        assert_eq!(
            (selected(&screen), screen.focused(), offset(&screen)),
            (row, Some(WidgetId(focused)), Some(expected_offset)),
            "{name}"
        );
    }
}

#[test]
fn double_click_and_enter_activate_the_row() {
    let cases = [
        ("double click in 100 ms", Some(100), None, Some(1)),
        ("two presses 400 ms apart", Some(400), None, None),
        ("enter", None, Some(GuiKey::Enter), Some(1)),
    ];
    for (name, second_press_ms, then_key, expected) in cases {
        let mut screen = selection_screen();
        let mut events = Vec::new();
        screen.mouse_press(
            PointerPress {
                x: 200.0,
                y: 100.0,
                button: PointerButton::Left,
                modifiers: KeyModifiers::NONE,
                now_ms: 0,
            },
            &mono,
            &mut events,
        );
        if let Some(now_ms) = second_press_ms {
            screen.mouse_press(
                PointerPress {
                    x: 200.0,
                    y: 100.0,
                    button: PointerButton::Left,
                    modifiers: KeyModifiers::NONE,
                    now_ms,
                },
                &mono,
                &mut events,
            );
        }
        if let Some(pressed) = then_key {
            events.extend(key(&mut screen, pressed, KeyModifiers::NONE));
        }
        let activated = events.iter().find_map(|event| match event {
            WidgetEvent::RowActivated { widget, row } if *widget == LIST => Some(*row),
            _ => None,
        });
        assert_eq!(activated, expected, "{name}");
    }
}

enum RowChange {
    Insert(usize),
    Remove(usize),
    Replace(usize),
}

#[test]
fn row_changes_move_the_selection_and_keep_the_bottom_distance() {
    let cases = [
        ("insert above", RowChange::Insert(0), Some(4), 126.0),
        ("remove the first", RowChange::Remove(0), Some(2), 54.0),
        ("remove the selected", RowChange::Remove(3), None, 90.0),
        ("insert below", RowChange::Insert(5), Some(3), 90.0),
        ("replace", RowChange::Replace(2), None, 0.0),
    ];
    for (name, change, row, expected_offset) in cases {
        let mut screen = selection_screen();
        screen.mouse_wheel(200.0, 100.0, -5.0, &mut Vec::new());
        press(&mut screen, 200.0, 60.0, PointerButton::Left);
        match change {
            RowChange::Insert(index) => screen.insert_row(LIST, index),
            RowChange::Remove(index) => screen.remove_row(LIST, index),
            RowChange::Replace(rows) => screen.set_rows(LIST, rows),
        }
        assert_eq!(
            (selected(&screen), offset(&screen)),
            (row, Some(expected_offset)),
            "{name}"
        );
    }
}

#[test]
fn rows_follow_the_spec_layout() {
    let mut options = WidgetList::options();
    for _ in 0..13 {
        options.push_row(25);
    }
    let first_row = options.row_rect(AREA, 0).expect("the row exists");
    let option_list = widget(1, AREA, WidgetKind::WidgetList(options));
    let selection = SelectionList::new(10, 36);
    let selection_list = widget(2, AREA, WidgetKind::SelectionList(selection.clone()));
    let scrollbar_x = |list: &Widget| list.scrollbar().map(|bar| bar.track.x);
    let cases = [
        (
            "option column 0",
            Some(WidgetList::option_origin(427, first_row, 0).x),
            58,
        ),
        (
            "option column 1",
            Some(WidgetList::option_origin(427, first_row, 1).x),
            218,
        ),
        (
            "option widget top",
            Some(WidgetList::option_origin(427, first_row, 0).y),
            37,
        ),
        ("option list scrollbar", scrollbar_x(&option_list), 376),
        (
            "selection row left",
            selection.row_rect(AREA, 0).map(|row| row.x),
            103,
        ),
        (
            "selection list scrollbar",
            scrollbar_x(&selection_list),
            331,
        ),
    ];
    for (name, actual, expected) in cases {
        assert_eq!(actual, Some(expected), "{name}");
    }
}

/// An option list of rows [10, 11], [12 wide], [13, 14] and [15, 16 inactive], and a button
/// below.
fn option_screen() -> Widgets {
    let rows: [&[(u32, i32)]; 4] = [
        &[(10, 150), (11, 150)],
        &[(12, 310)],
        &[(13, 150), (14, 150)],
        &[(15, 150), (16, 150)],
    ];
    let mut list = WidgetList::options();
    let mut members = Vec::new();
    for (row, row_widgets) in rows.iter().enumerate() {
        list.push_row(25);
        let Some(row_rect) = list.row_rect(AREA, row) else {
            continue;
        };
        for (column, (id, width)) in row_widgets.iter().enumerate() {
            let origin = WidgetList::option_origin(427, row_rect, column as i32);
            let mut member = button(*id, origin.x, origin.y, *width, 20);
            member.area = Some(AreaSlot {
                area: WidgetId(1),
                row,
            });
            member.active = *id != 16;
            members.push(member);
        }
    }
    let mut widgets = vec![widget(1, AREA, WidgetKind::WidgetList(list))];
    widgets.extend(members);
    widgets.push(button(2, 163, 210, 100, 20));
    screen_of(widgets)
}

#[test]
fn arrows_and_tab_move_through_an_option_list() {
    let cases: [(&str, Option<u32>, &[_], u32); 11] = [
        ("right in a row", Some(10), &[RIGHT], 11),
        ("down to a row of one", Some(10), &[RIGHT, DOWN], 12),
        ("down keeps the new column", Some(11), &[DOWN, DOWN], 13),
        (
            "down onto an inactive widget leaves the list",
            Some(14),
            &[DOWN],
            2,
        ),
        ("up to a row of one", Some(14), &[UP], 12),
        ("left at the start of a row", Some(13), &[LEFT], 13),
        ("right at the end of a row", Some(11), &[RIGHT], 11),
        (
            "tab through the rows",
            None,
            &[TAB, TAB, TAB, TAB, TAB, TAB],
            15,
        ),
        (
            "tab out of the list",
            None,
            &[TAB, TAB, TAB, TAB, TAB, TAB, TAB],
            2,
        ),
        ("shift tab into the last row", None, &[BACK, BACK], 15),
        ("up from below enters the last row", None, &[BACK, UP], 15),
    ];
    for (name, pressed_widget, pressed_keys, expected) in cases {
        let mut screen = option_screen();
        if let Some(id) = pressed_widget {
            let rect = screen.get(WidgetId(id)).expect("the widget exists").rect();
            press(
                &mut screen,
                f64::from(rect.x) + 1.0,
                f64::from(rect.y) + 1.0,
                PointerButton::Left,
            );
        }
        keys(&mut screen, pressed_keys);
        assert_eq!(screen.focused(), Some(WidgetId(expected)), "{name}");
    }
}

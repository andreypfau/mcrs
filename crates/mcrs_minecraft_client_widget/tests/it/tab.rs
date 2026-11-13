use mcrs_minecraft_client_widget::{
    GuiExtent, GuiKey, GuiPoint, GuiRect, KeyModifiers, PointerButton, Tab, TabBar, WidgetEvent,
    WidgetId, WidgetKind, Widgets,
};

use crate::screen::{SCREEN, button, key, press, screen_of, widget};

const BAR: WidgetId = WidgetId(1);

fn tabs(active: &[bool]) -> TabBar {
    TabBar::new(
        active
            .iter()
            .map(|active| Tab {
                label: String::new(),
                active: *active,
            })
            .collect(),
    )
}

fn control_shift() -> KeyModifiers {
    let mut modifiers = KeyModifiers::CONTROL;
    modifiers.shift = true;
    modifiers
}

#[test]
fn menu_tabs_share_the_bar_in_even_widths() {
    let cases = [
        (427, [28, 152, 276], 124, 400),
        (320, [14, 112, 210], 98, 308),
        (20, [10, 10, 10], 0, 10),
    ];
    for (screen_width, lefts, width, right) in cases {
        let bar = tabs(&[true, true, true]);
        let rect = bar.menu_rect(screen_width);
        let tab_rects: Vec<_> = (0..3).filter_map(|tab| bar.tab_rect(rect, tab)).collect();
        assert_eq!(
            tab_rects.iter().map(|tab| tab.x).collect::<Vec<_>>(),
            lefts,
            "W = {screen_width}"
        );
        assert!(
            tab_rects
                .iter()
                .all(|tab| tab.width == width && tab.height == 24),
            "W = {screen_width}"
        );
        assert_eq!(rect.x + rect.width, right, "W = {screen_width}");
    }
}

#[test]
fn current_tab_draws_its_background_line_and_label_higher() {
    let bar = tabs(&[true, true, true]);
    let rect = bar.menu_rect(427);
    let [left, right] = TabBar::separators(rect, 427);
    let cases = [
        (
            "background",
            bar.current_background(rect),
            GuiRect::new(30, 2, 120, 22),
        ),
        (
            "line of a short label",
            bar.current_line(rect, 40),
            GuiRect::new(70, 22, 40, 1),
        ),
        (
            "line of a long label",
            bar.current_line(rect, 200),
            GuiRect::new(30, 22, 120, 1),
        ),
        (
            "line left of the tabs",
            Some(left),
            GuiRect::new(0, 22, 28, 2),
        ),
        (
            "line right of the tabs",
            Some(right),
            GuiRect::new(400, 22, 27, 2),
        ),
        (
            "tab content area",
            Some(TabBar::content_area(SCREEN, 33)),
            GuiRect::new(0, 24, 427, 183),
        ),
    ];
    for (name, actual, expected) in cases {
        assert_eq!(actual, Some(expected), "{name}");
    }
    assert_eq!(
        TabBar::content_origin(SCREEN, 33, GuiExtent::new(200, 100)),
        GuiPoint::new(113, 37),
        "layout of a tab"
    );
    let labels = [(0, GuiPoint::new(70, 8)), (1, GuiPoint::new(194, 10))];
    for (tab, expected) in labels {
        let origin = bar
            .label_layout(rect, tab, 40, 0)
            .map(|layout| layout.origin);
        assert_eq!(origin, Some(expected), "label of tab {tab}");
    }
}

/// A menu tab bar at W = 427 and a button under it.
fn tab_screen(active: &[bool], current: usize) -> Widgets {
    let mut bar = tabs(active);
    bar.set_current(current);
    let rect = bar.menu_rect(427);
    screen_of([
        widget(BAR.0, rect, WidgetKind::TabBar(bar)),
        button(2, 163, 100, 100, 20),
    ])
}

fn current(screen: &Widgets) -> Option<usize> {
    match screen.get(BAR).map(|bar| &bar.kind) {
        Some(WidgetKind::TabBar(bar)) => Some(bar.current()),
        _ => None,
    }
}

fn changes(events: &[WidgetEvent]) -> usize {
    events
        .iter()
        .filter(|event| matches!(event, WidgetEvent::TabChanged { widget, .. } if *widget == BAR))
        .count()
}

const ALL: &[bool] = &[true, true, true];
const MIDDLE_OFF: &[bool] = &[true, false, true];
const ONLY_FIRST: &[bool] = &[true, false, false];
const TAB: (GuiKey, KeyModifiers) = (GuiKey::Tab, KeyModifiers::NONE);
const CTRL_TAB: (GuiKey, KeyModifiers) = (GuiKey::Tab, KeyModifiers::CONTROL);

/// The current tab, the focused widget and the number of tab changes after a press and keys.
fn run(
    active: &[bool],
    start: usize,
    pressed_at: Option<(f64, f64)>,
    pressed_keys: &[(GuiKey, KeyModifiers)],
) -> (Option<usize>, Option<WidgetId>, usize) {
    let mut screen = tab_screen(active, start);
    let mut events = Vec::new();
    if let Some((x, y)) = pressed_at {
        events.extend(press(&mut screen, x, y, PointerButton::Left));
    }
    for (pressed, modifiers) in pressed_keys {
        events.extend(key(&mut screen, *pressed, *modifiers));
    }
    (current(&screen), screen.focused(), changes(&events))
}

#[test]
fn ctrl_keys_change_the_current_tab() {
    let ctrl = |digit| (GuiKey::Digit(digit), KeyModifiers::CONTROL);
    let cases = [
        ("ctrl 1", ALL, 2, ctrl(1), 0, 1),
        ("ctrl 3", ALL, 0, ctrl(3), 2, 1),
        ("ctrl 0 gives the last tab", ALL, 0, ctrl(0), 2, 1),
        ("ctrl 9 gives the last tab", ALL, 0, ctrl(9), 2, 1),
        ("ctrl onto an inactive tab", MIDDLE_OFF, 0, ctrl(2), 0, 0),
        (
            "digit without ctrl",
            ALL,
            0,
            (GuiKey::Digit(2), KeyModifiers::NONE),
            0,
            0,
        ),
        ("ctrl tab wraps", ALL, 2, CTRL_TAB, 0, 1),
        (
            "ctrl shift tab wraps",
            ALL,
            0,
            (GuiKey::Tab, control_shift()),
            2,
            1,
        ),
        (
            "ctrl tab skips an inactive tab",
            MIDDLE_OFF,
            0,
            CTRL_TAB,
            2,
            1,
        ),
        (
            "ctrl tab with no other active tab",
            ONLY_FIRST,
            0,
            CTRL_TAB,
            0,
            0,
        ),
    ];
    for (name, active, start, pressed, expected, expected_changes) in cases {
        let (tab, _, changes) = run(active, start, None, &[pressed]);
        assert_eq!((tab, changes), (Some(expected), expected_changes), "{name}");
    }
}

#[test]
fn presses_and_arrows_change_the_current_tab() {
    let right = (GuiKey::Right, KeyModifiers::NONE);
    let cases: [(_, _, _, &[_], _, _, _); 8] = [
        (
            "ctrl tab keeps the focus",
            ALL,
            Some((200.0, 105.0)),
            &[CTRL_TAB],
            1,
            2,
            1,
        ),
        ("press on a tab", ALL, Some((300.0, 10.0)), &[], 2, 1, 1),
        (
            "press on the current tab",
            ALL,
            Some((50.0, 10.0)),
            &[],
            0,
            1,
            0,
        ),
        (
            "right moves the focused tab",
            ALL,
            None,
            &[TAB, right],
            1,
            1,
            1,
        ),
        (
            "right skips an inactive tab",
            MIDDLE_OFF,
            None,
            &[TAB, right],
            2,
            1,
            1,
        ),
        (
            "right at the last tab",
            ALL,
            None,
            &[TAB, right, right, right],
            2,
            1,
            2,
        ),
        (
            "left at the first tab",
            ALL,
            None,
            &[TAB, (GuiKey::Left, KeyModifiers::NONE)],
            0,
            1,
            0,
        ),
        (
            "down leaves the bar and up comes back",
            ALL,
            None,
            &[
                TAB,
                (GuiKey::Down, KeyModifiers::NONE),
                (GuiKey::Up, KeyModifiers::NONE),
            ],
            0,
            1,
            0,
        ),
    ];
    for (name, active, pressed_at, pressed_keys, tab, focused, expected_changes) in cases {
        assert_eq!(
            run(active, 0, pressed_at, pressed_keys),
            (Some(tab), Some(WidgetId(focused)), expected_changes),
            "{name}"
        );
    }
    assert_eq!(
        run(MIDDLE_OFF, 0, Some((200.0, 10.0)), &[]),
        (Some(0), None, 0),
        "a press on an inactive tab"
    );
}

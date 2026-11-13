use mcrs_minecraft_client_widget::{
    AreaSlot, GuiInput, GuiPoint, GuiRect, PointerButton, ScrollPanel, SelectionList, Widget,
    WidgetEvent, WidgetId, WidgetKind, WidgetList, Widgets,
};

use crate::screen::{AWAY, button, mono, press, screen_of, screen_with_input, widget};

const AREA: GuiRect = GuiRect::new(0, 33, 427, 168);
const LIST: WidgetId = WidgetId(1);

fn options(rows: usize) -> WidgetList {
    let mut list = WidgetList::options();
    for _ in 0..rows {
        list.push_row(25);
    }
    list
}

fn option_list(rows: usize) -> WidgetKind {
    WidgetKind::WidgetList(options(rows))
}

fn panel(area: GuiRect, content_height: i32) -> Widget {
    widget(
        LIST.0,
        area,
        WidgetKind::ScrollPanel(ScrollPanel::new(content_height)),
    )
}

fn wheel(screen: &mut Widgets, v: f64) {
    screen.mouse_wheel(200.0, 100.0, v, &mut Vec::new());
}

fn offset(screen: &Widgets) -> Option<f64> {
    screen.get(LIST).and_then(Widget::scroll_offset)
}

#[test]
fn wheel_moves_the_offset_by_the_step_within_the_limits() {
    let cases = [
        ("option list down", option_list(13), -1.0, 12.0),
        ("option list up at the top", option_list(13), 1.0, 0.0),
        ("option list to the bottom", option_list(13), -50.0, 161.0),
        (
            "rows of 36",
            WidgetKind::SelectionList(SelectionList::new(10, 36)),
            -1.0,
            18.0,
        ),
        (
            "not scrollable",
            WidgetKind::SelectionList(SelectionList::new(2, 36)),
            -1.0,
            0.0,
        ),
        (
            "panel",
            WidgetKind::ScrollPanel(ScrollPanel::new(340)),
            -1.0,
            10.0,
        ),
        (
            "panel to the bottom",
            WidgetKind::ScrollPanel(ScrollPanel::new(340)),
            -100.0,
            172.0,
        ),
    ];
    for (name, kind, v, expected) in cases {
        let mut screen = screen_of([widget(LIST.0, AREA, kind)]);
        wheel(&mut screen, v);
        assert_eq!(offset(&screen), Some(expected), "{name}");
    }
}

#[test]
fn thumb_follows_the_offset() {
    let cases = [
        ("top", 168, 340, 0.0, Some((33, 83))),
        ("spec example at s = 86", 168, 340, -8.6, Some((75, 83))),
        ("bottom", 168, 340, -100.0, Some((118, 83))),
        ("minimum height", 168, 10_000, 0.0, Some((33, 32))),
        (
            "eight units short of the track",
            100,
            101,
            0.0,
            Some((33, 92)),
        ),
        ("not scrollable", 168, 168, 0.0, None),
    ];
    for (name, height, content, v, expected) in cases {
        let mut screen = screen_of([panel(GuiRect::new(0, 33, 427, height), content)]);
        wheel(&mut screen, v);
        let scrollbar = screen.get(LIST).and_then(Widget::scrollbar);
        let expected = expected.map(|(y, h)| GuiRect::new(421, y, 6, h));
        assert_eq!(scrollbar.map(|bar| bar.thumb), expected, "{name}");
    }
}

#[test]
fn scrollbar_drag_moves_the_offset() {
    let dragged_17 = 17.0 * 172.0 / 85.0;
    let cases = [
        ("press on the track below the thumb", 150, None, 0.0),
        (
            "track, then down by 17",
            150,
            Some(GuiPoint::new(422, 167)),
            dragged_17,
        ),
        (
            "thumb, then down by 17",
            50,
            Some(GuiPoint::new(422, 67)),
            dragged_17,
        ),
        (
            "off the bar to the left",
            50,
            Some(GuiPoint::new(100, 67)),
            dragged_17,
        ),
        ("above the area", 50, Some(GuiPoint::new(422, 10)), 0.0),
        ("below the area", 50, Some(GuiPoint::new(422, 230)), 172.0),
    ];
    for (name, press_y, to, expected) in cases {
        let mut screen = screen_with_input(GuiInput::new(AWAY), [panel(AREA, 340)]);
        screen.mouse_move(GuiPoint::new(422, press_y));
        press(&mut screen, 422.0, f64::from(press_y), PointerButton::Left);
        if let Some(to) = to {
            screen.mouse_move(to);
            screen.mouse_drag(f64::from(to.x), 0, &mono, &mut Vec::new());
        }
        assert_eq!(offset(&screen), Some(expected), "{name}");
    }
}

fn scrolled_option_list() -> Widgets {
    let list = options(13);
    let mut widgets = vec![widget(LIST.0, AREA, WidgetKind::WidgetList(list.clone()))];
    for row in 0..13 {
        let Some(row_rect) = list.row_rect(AREA, row) else {
            continue;
        };
        for column in 0..2 {
            let origin = WidgetList::option_origin(427, row_rect, column);
            let id = 100 + 2 * row as u32 + column as u32;
            let mut member = button(id, origin.x, origin.y, 150, 20);
            member.area = Some(AreaSlot { area: LIST, row });
            widgets.push(member);
        }
    }
    let mut screen = screen_of(widgets);
    wheel(&mut screen, -1.0);
    screen
}

#[test]
fn press_goes_through_the_clip() {
    let cases = [
        (
            "row 0, column 0, scrolls to the top",
            100.0,
            40.0,
            Some(100),
            0.0,
        ),
        ("row 0, column 1", 300.0, 40.0, Some(101), 0.0),
        ("between the columns", 212.0, 40.0, None, 12.0),
        (
            "row 7 above the bottom, scrolls it in",
            100.0,
            200.5,
            Some(114),
            36.0,
        ),
        ("row 7 below the bottom", 100.0, 205.0, None, 12.0),
    ];
    for (name, x, y, activated, expected_offset) in cases {
        let mut screen = scrolled_option_list();
        let events = press(&mut screen, x, y, PointerButton::Left);
        let pressed = events.iter().find_map(|event| match event {
            WidgetEvent::Activated { widget, .. } => Some(widget.0),
            _ => None,
        });
        assert_eq!(pressed, activated, "{name}");
        assert_eq!(offset(&screen), Some(expected_offset), "{name}");
    }
}

#[test]
fn hover_is_cut_to_the_clip() {
    let cases = [
        ("row 0 inside the list", 40, Some(100)),
        ("row 0 above the list", 30, None),
        ("row 7 inside the list", 200, Some(114)),
        ("row 7 below the list", 202, None),
    ];
    for (name, y, expected) in cases {
        let mut screen = scrolled_option_list();
        screen.mouse_move(GuiPoint::new(100, y));
        let hovered = screen
            .iter()
            .filter(|widget| widget.area.is_some() && screen.is_hovered(widget))
            .map(|widget| widget.id().0)
            .next();
        assert_eq!(hovered, expected, "{name}");
    }
}

#[test]
fn an_offset_set_from_outside_or_by_new_bounds_stays_within_the_limits() {
    enum Change {
        Offset(f64),
        Bounds(GuiRect),
    }
    let cases = [
        ("below the top", Change::Offset(-5.0), 0.0),
        ("inside", Change::Offset(50.0), 50.0),
        ("beyond the bottom", Change::Offset(1000.0), 161.0),
        (
            "a taller list",
            Change::Bounds(GuiRect::new(0, 33, 427, 300)),
            29.0,
        ),
    ];
    for (name, change, expected) in cases {
        let mut screen = screen_of([widget(LIST.0, AREA, option_list(13))]);
        wheel(&mut screen, -50.0);
        match change {
            Change::Offset(offset) => screen.set_scroll_offset(LIST, offset),
            Change::Bounds(rect) => screen.set_rect(LIST, rect),
        }
        assert_eq!(offset(&screen), Some(expected), "{name}");
    }
}

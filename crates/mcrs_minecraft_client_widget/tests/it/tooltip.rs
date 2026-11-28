use mcrs_minecraft_client_widget::{
    GuiExtent, GuiKey, GuiPoint, GuiRect, KeyModifiers, PointerButton, TooltipAnchor, TooltipDelay,
    TooltipRequest, WidgetId,
};

use crate::screen::{SCREEN, button, key, press, screen_of};

const CONTENT: GuiExtent = GuiExtent::new(50, 9);

fn with_tooltip(id: u32, delay_ms: u32) -> mcrs_minecraft_client_widget::Widget {
    let mut widget = button(id, 100, 100, 150, 20);
    widget.tooltip = Some(TooltipDelay { delay_ms });
    widget
}

#[test]
fn a_tooltip_shows_once_more_than_its_delay_passed_since_the_start_of_the_want() {
    let cases: [(u32, &[(u64, bool)]); 2] = [
        (0, &[(0, false), (0, false), (1, true)]),
        (500, &[(1000, false), (1500, false), (1501, true)]),
    ];
    for (delay_ms, frames) in cases {
        let mut screen = screen_of([with_tooltip(1, delay_ms)]);
        screen.mouse_move(GuiPoint::new(110, 110));
        for &(now_ms, shows) in frames {
            assert_eq!(
                screen.tooltip(now_ms).is_some(),
                shows,
                "delay {delay_ms} at {now_ms} ms"
            );
        }
    }
}

#[test]
fn a_frame_without_the_want_restarts_the_delay() {
    let mut screen = screen_of([with_tooltip(1, 100)]);
    screen.mouse_move(GuiPoint::new(110, 110));
    assert!(screen.tooltip(0).is_none());
    screen.mouse_move(GuiPoint::new(110, 300));
    assert!(screen.tooltip(150).is_none());
    screen.mouse_move(GuiPoint::new(110, 110));
    assert!(screen.tooltip(200).is_none());
    assert!(screen.tooltip(300).is_none());
    assert!(screen.tooltip(301).is_some());
}

#[test]
fn a_focused_widget_replaces_the_request_of_an_earlier_widget() {
    let mut first = with_tooltip(1, 0);
    first.active = false;
    let mut screen = screen_of([first, with_tooltip(2, 0)]);
    screen.mouse_move(GuiPoint::new(110, 110));
    screen.tooltip(0);
    assert_eq!(
        screen.tooltip(1).map(|request| request.widget),
        Some(WidgetId(1))
    );
    press(&mut screen, 110.0, 110.0, PointerButton::Left);
    assert_eq!(
        screen.tooltip(2).map(|request| request.widget),
        Some(WidgetId(2))
    );
}

#[test]
fn a_focused_widget_after_a_key_asks_for_its_tooltip_below_itself() {
    let mut screen = screen_of([with_tooltip(1, 0)]);
    key(&mut screen, GuiKey::Tab, KeyModifiers::NONE);
    screen.tooltip(0);
    assert_eq!(
        screen.tooltip(1),
        Some(TooltipRequest {
            widget: WidgetId(1),
            bounds: GuiRect::new(100, 100, 150, 20),
            screen: SCREEN,
            anchor: TooltipAnchor::Focus,
        })
    );
}

#[test]
fn the_tooltip_of_a_widget_sits_by_the_mouse_or_by_the_focused_widget() {
    let at = |bounds, anchor| TooltipRequest {
        widget: WidgetId(1),
        bounds,
        screen: SCREEN,
        anchor,
    };
    let mouse = |x, y| TooltipAnchor::Mouse(GuiPoint::new(x, y));
    let middle = GuiRect::new(100, 100, 150, 20);
    let cases = [
        (at(middle, mouse(120, 100)), GuiPoint::new(132, 118)),
        (at(middle, mouse(120, 119)), GuiPoint::new(132, 127)),
        (at(middle, mouse(120, 110)), GuiPoint::new(132, 122)),
        (at(middle, mouse(400, 110)), GuiPoint::new(338, 122)),
        (
            at(GuiRect::new(100, 200, 150, 20), mouse(120, 210)),
            GuiPoint::new(132, 185),
        ),
        (at(middle, TooltipAnchor::Focus), GuiPoint::new(103, 124)),
        (
            at(GuiRect::new(100, 220, 150, 20), TooltipAnchor::Focus),
            GuiPoint::new(103, 207),
        ),
        (
            at(GuiRect::new(400, 100, 20, 20), TooltipAnchor::Focus),
            GuiPoint::new(367, 124),
        ),
    ];
    for (request, expected) in cases {
        assert_eq!(request.origin(CONTENT), expected, "{request:?}");
    }
}

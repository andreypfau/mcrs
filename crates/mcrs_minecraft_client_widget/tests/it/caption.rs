use mcrs_minecraft_client_widget::{
    GuiExtent, GuiPoint, GuiRect, IconForm, Overflow, WidgetKind, checkbox_height,
    checkbox_label_origin, checkbox_width, text_line_extent, text_lines_extent,
};

use crate::screen::widget;

const BUTTON: GuiRect = GuiRect::new(0, 0, 150, 20);
const AREA: GuiRect = GuiRect::new(2, 0, 146, 20);

fn origin(kind: WidgetKind, rect: GuiRect, text_width: i32, now_ms: u64) -> Option<GuiPoint> {
    widget(1, rect, kind)
        .caption_layout(text_width, now_ms)
        .map(|layout| layout.origin)
}

#[test]
fn a_caption_that_fits_centres_in_its_label_area() {
    let icon = WidgetKind::IconButton {
        icon: GuiExtent::new(15, 15),
        form: IconForm::IconRight,
    };
    let cases = [
        (WidgetKind::Button, 40, GuiPoint::new(55, 6)),
        (WidgetKind::Button, 146, GuiPoint::new(2, 6)),
        (icon.clone(), 40, GuiPoint::new(55, 6)),
        (icon, 120, GuiPoint::new(11, 6)),
    ];
    for (kind, text_width, expected) in cases {
        assert_eq!(
            origin(kind.clone(), BUTTON, text_width, 0),
            Some(expected),
            "{kind:?} with a caption of {text_width}"
        );
    }
}

#[test]
fn a_caption_wider_than_its_area_scrolls_from_its_end_to_its_start_and_back() {
    let cases = [
        (156, 0, -8),
        (156, 1250, -3),
        (156, 2500, 2),
        (156, 5000, -8),
        (150, 1500, 2),
        (150, 3000, -2),
    ];
    for (text_width, now_ms, x) in cases {
        let layout = widget(1, BUTTON, WidgetKind::Button).caption_layout(text_width, now_ms);
        assert_eq!(
            layout.map(|layout| (layout.origin, layout.clip)),
            Some((GuiPoint::new(x, 6), Some(AREA))),
            "a caption of {text_width} at {now_ms} ms"
        );
    }
}

#[test]
fn an_icon_draws_centred_at_the_right_or_after_a_caption_that_leaves_room() {
    let cases = [
        (
            IconForm::IconOnly,
            GuiRect::new(10, 10, 20, 20),
            0,
            GuiPoint::new(13, 13),
        ),
        (IconForm::IconRight, BUTTON, 40, GuiPoint::new(133, 3)),
        (IconForm::IconAfterText, BUTTON, 40, GuiPoint::new(99, 3)),
        (IconForm::IconAfterText, BUTTON, 130, GuiPoint::new(144, 3)),
        (IconForm::IconAfterText, BUTTON, 131, GuiPoint::new(133, 3)),
    ];
    for (form, rect, text_width, expected) in cases {
        let kind = WidgetKind::IconButton {
            icon: GuiExtent::new(15, 15),
            form,
        };
        assert_eq!(
            widget(1, rect, kind).icon_origin(text_width),
            Some(expected),
            "{form:?} with a caption of {text_width}"
        );
    }
}

#[test]
fn a_checkbox_is_its_label_plus_twenty_one_and_holds_at_most_two_label_lines() {
    assert_eq!(checkbox_width(40, None), 61);
    assert_eq!(checkbox_width(40, Some(50)), 50);
    let cases = [(1, 17, 4), (2, 18, -1), (3, 18, -1)];
    for (lines, height, label_y) in cases {
        assert_eq!(checkbox_height(lines), height, "{lines} lines");
        assert_eq!(
            checkbox_label_origin(GuiRect::new(10, 0, 61, 17), lines),
            GuiPoint::new(31, label_y),
            "{lines} lines"
        );
    }
}

#[test]
fn a_one_line_text_starts_at_its_left_edge_and_scrolls_only_past_its_limit() {
    assert_eq!(text_line_extent(80, None), GuiExtent::new(80, 9));
    assert_eq!(text_line_extent(80, Some(50)), GuiExtent::new(50, 9));
    let rect = GuiRect::new(10, 20, 50, 20);
    let text = |overflow| WidgetKind::Text {
        max_width: None,
        overflow,
    };
    assert_eq!(
        origin(text(Overflow::Scroll), rect, 50, 0),
        Some(GuiPoint::new(10, 25))
    );
    assert_eq!(
        origin(text(Overflow::Cut), rect, 80, 0),
        Some(GuiPoint::new(10, 25))
    );
    assert_eq!(
        widget(1, rect, text(Overflow::Scroll))
            .caption_layout(80, 0)
            .map(|layout| (layout.origin, layout.clip)),
        Some((
            GuiPoint::new(12 - 34, 26),
            Some(GuiRect::new(12, 20, 46, 20))
        ))
    );
}

#[test]
fn a_multi_line_text_stacks_its_lines_and_centres_each_on_request() {
    assert_eq!(text_lines_extent([30, 50, 10]), GuiExtent::new(50, 27));
    assert_eq!(text_lines_extent([]), GuiExtent::new(0, 0));
    let rect = GuiRect::new(0, 0, 50, 27);
    let lines = |centred| WidgetKind::MultiLineText {
        max_width: None,
        centred,
    };
    assert_eq!(
        widget(1, rect, lines(true)).line_origin(2, 10),
        Some(GuiPoint::new(20, 18))
    );
    assert_eq!(
        widget(1, rect, lines(false)).line_origin(2, 10),
        Some(GuiPoint::new(0, 18))
    );
}

#[test]
fn only_an_inactive_button_greys_its_caption() {
    let cases = [
        (WidgetKind::Button, true, 0xFFFF_FFFF),
        (WidgetKind::Button, false, 0xFFA0_A0A0),
        (WidgetKind::Checkbox { selected: false }, false, 0xFFFF_FFFF),
    ];
    for (kind, active, color) in cases {
        let mut button = widget(1, BUTTON, kind.clone());
        button.active = active;
        assert_eq!(button.caption_color(), color, "{kind:?} active {active}");
    }
}

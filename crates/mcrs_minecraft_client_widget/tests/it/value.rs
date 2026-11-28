use mcrs_minecraft_client_widget::{
    CycleButton, EditBox, GuiRect, Slider, WidgetId, WidgetKind, WidgetValue,
};

use crate::screen::{button, screen_of, widget};

#[test]
fn an_input_widget_reports_its_value_and_the_others_report_none() {
    let mut edit = EditBox::default();
    edit.insert("seed");
    let rect = GuiRect::new(0, 0, 100, 20);
    let screen = screen_of([
        widget(1, rect, WidgetKind::EditBox(edit)),
        widget(2, rect, WidgetKind::Slider(Slider::continuous(0.25))),
        widget(
            3,
            rect,
            WidgetKind::Slider(Slider::range(30, 110, 70).unwrap()),
        ),
        widget(
            4,
            rect,
            WidgetKind::CycleButton(CycleButton::new(3, 2).unwrap()),
        ),
        widget(7, rect, WidgetKind::Checkbox { selected: true }),
        widget(8, rect, WidgetKind::Slider(Slider::list(4, 3).unwrap())),
        button(5, 0, 0, 10, 10),
    ]);
    let cases = [
        (1, Some(WidgetValue::Text("seed"))),
        (2, Some(WidgetValue::Number(0.25))),
        (3, Some(WidgetValue::Int(70))),
        (4, Some(WidgetValue::Choice(2))),
        (7, Some(WidgetValue::Bool(true))),
        (8, Some(WidgetValue::Choice(3))),
        (5, None),
        (6, None),
    ];
    for (id, expected) in cases {
        assert_eq!(screen.value(WidgetId(id)), expected, "widget {id}");
    }
}

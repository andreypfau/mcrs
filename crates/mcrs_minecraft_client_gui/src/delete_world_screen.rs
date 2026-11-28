use mcrs_minecraft_client_widget::{
    Alignment, BUTTON_HEIGHT, BUTTON_WIDTH, Child, GuiExtent, GuiRect, LayoutCell, Overflow,
    Padding, Stack, Widget, WidgetId, WidgetKind, text_line_extent, text_lines_extent,
};

use crate::gui_quad::WHITE;
use mcrs_minecraft_font::font::{Font, Span};
use mcrs_minecraft_font::language::ClientLanguage;
use mcrs_minecraft_font::text::{self, Wrap};

const TITLE: WidgetId = WidgetId(0);
const MESSAGE: WidgetId = WidgetId(1);
pub(crate) const DELETE: WidgetId = WidgetId(2);
pub(crate) const CANCEL: WidgetId = WidgetId(3);

const COLUMN_GAP: i32 = 8;
const BUTTON_GAP: i32 = 4;
const BUTTON_PADDING_TOP: i32 = 16;
const MESSAGE_MARGIN: i32 = 50;
const MESSAGE_ROWS: usize = 15;

pub(crate) fn delete_world_screen(
    screen: GuiExtent,
    language: &ClientLanguage,
    font: &Font,
    name: &str,
) -> Option<[Widget; 4]> {
    let title = language.get("selectWorld.deleteQuestion");
    let warning = language.translate(
        "selectWorld.deleteWarning",
        WHITE,
        &[vec![Span::new(name, WHITE)]],
    );
    let width = screen.width - MESSAGE_MARGIN;
    let mut rows = Vec::new();
    text::wrap(&warning, width, Wrap::Tooltip, font, &mut rows);
    rows.truncate(MESSAGE_ROWS);
    let lines: Vec<String> = rows
        .iter()
        .map(|row| row.iter().map(|span| span.text.as_str()).collect())
        .collect();
    let across = |top| LayoutCell {
        padding: Padding {
            top,
            ..Padding::default()
        },
        alignment: Alignment { x: 0.5, y: 0.0 },
    };
    let button = Child::new(
        GuiExtent::new(BUTTON_WIDTH, BUTTON_HEIGHT),
        across(BUTTON_PADDING_TOP),
    );
    let buttons = [button, button];
    let buttons = Stack::row(BUTTON_GAP, &buttons);
    let children = [
        Child::new(text_line_extent(font.str_width(title), None), across(0)),
        Child::new(
            text_lines_extent(lines.iter().map(|line| font.str_width(line))),
            across(0),
        ),
        Child::new(buttons.size(), across(0)),
    ];
    let column = Stack::column(COLUMN_GAP, &children);
    let at =
        GuiRect::new(0, 0, screen.width, screen.height).align(column.size(), Alignment::CENTER);
    let mut rects = column.rects(at.x, at.y);
    let (title_rect, message_rect, row) = (rects.next()?, rects.next()?, rects.next()?);
    let mut buttons = buttons.rects(row.x, row.y);
    Some([
        Widget::new(
            TITLE,
            title_rect,
            title.to_owned(),
            WidgetKind::Text {
                max_width: None,
                overflow: Overflow::Cut,
            },
        ),
        Widget::new(
            MESSAGE,
            message_rect,
            lines.join("\n"),
            WidgetKind::MultiLineText {
                max_width: Some(width),
                centred: true,
            },
        ),
        Widget::new(
            DELETE,
            buttons.next()?,
            language.get("selectWorld.deleteButton").to_owned(),
            WidgetKind::Button,
        ),
        Widget::new(
            CANCEL,
            buttons.next()?,
            language.get("gui.cancel").to_owned(),
            WidgetKind::Button,
        ),
    ])
}

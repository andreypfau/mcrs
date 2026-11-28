use mcrs_minecraft_client_widget::{
    Alignment, BUTTON_HEIGHT, Child, GuiExtent, GuiRect, LayoutCell, Overflow, Padding, Stack,
    Widget, WidgetId, WidgetKind, text_line_extent, text_lines_extent,
};

use mcrs_minecraft_font::font::{Font, Span};
use mcrs_minecraft_font::language::ClientLanguage;
use mcrs_minecraft_font::text::{self, Wrap};

pub(crate) const TITLE: WidgetId = WidgetId(0);
pub(crate) const REASON: WidgetId = WidgetId(1);
pub(crate) const BACK: WidgetId = WidgetId(2);

const TEXT_PADDING: i32 = 10;
const BUTTON_PADDING: i32 = 2;
const REASON_MARGIN: i32 = 50;
const BUTTON_WIDTH: i32 = 200;

pub(crate) fn disconnected_screen(
    screen: GuiExtent,
    language: &ClientLanguage,
    font: &Font,
    title_key: &str,
    reason: &[Span],
) -> Option<[Widget; 3]> {
    let title = language.get(title_key);
    let mut rows = Vec::new();
    text::wrap(
        reason,
        screen.width - REASON_MARGIN,
        Wrap::Tooltip,
        font,
        &mut rows,
    );
    let lines: Vec<String> = rows
        .iter()
        .map(|row| row.iter().map(|span| span.text.as_str()).collect())
        .collect();
    let cell = |padding| LayoutCell {
        padding: Padding {
            left: padding,
            top: padding,
            right: padding,
            bottom: padding,
        },
        alignment: Alignment { x: 0.5, y: 0.0 },
    };
    let children = [
        Child::new(
            text_line_extent(font.str_width(title), None),
            cell(TEXT_PADDING),
        ),
        Child::new(
            text_lines_extent(lines.iter().map(|line| font.str_width(line))),
            cell(TEXT_PADDING),
        ),
        Child::new(
            GuiExtent::new(BUTTON_WIDTH, BUTTON_HEIGHT),
            cell(BUTTON_PADDING),
        ),
    ];
    let column = Stack::column(0, &children);
    let at =
        GuiRect::new(0, 0, screen.width, screen.height).align(column.size(), Alignment::CENTER);
    let mut rects = column.rects(at.x, at.y);
    Some([
        Widget::new(
            TITLE,
            rects.next()?,
            title.to_owned(),
            WidgetKind::Text {
                max_width: None,
                overflow: Overflow::Cut,
            },
        ),
        Widget::new(
            REASON,
            rects.next()?,
            lines.join("\n"),
            WidgetKind::MultiLineText {
                max_width: Some(screen.width - REASON_MARGIN),
                centred: true,
            },
        ),
        Widget::new(
            BACK,
            rects.next()?,
            language.get("gui.toTitle").to_owned(),
            WidgetKind::Button,
        ),
    ])
}

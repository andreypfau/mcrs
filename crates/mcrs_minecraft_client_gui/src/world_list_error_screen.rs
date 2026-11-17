use mcrs_minecraft_client_widget::{
    BUTTON_HEIGHT, GuiExtent, GuiRect, LINE_HEIGHT, Overflow, Widget, WidgetId, WidgetKind,
    text_lines_extent,
};

use crate::font::{Font, Span};
use crate::item_decorations::WHITE;
use crate::language::ClientLanguage;
use crate::text::{self, Wrap};

const TITLE: WidgetId = WidgetId(0);
const REASON: WidgetId = WidgetId(1);
pub(crate) const CANCEL: WidgetId = WidgetId(2);

const TITLE_Y: i32 = 90;
const REASON_Y: i32 = 110;
const REASON_MARGIN: i32 = 50;
const BUTTON_Y: i32 = 140;
const BUTTON_WIDTH: i32 = 200;

pub(crate) fn world_list_error_screen(
    screen: GuiExtent,
    language: &ClientLanguage,
    font: &Font,
) -> [Widget; 3] {
    let title = language.get("selectWorld.unable_to_load");
    let reason = [Span::new(
        language.get("selectWorld.load_folder_access"),
        WHITE,
    )];
    let width = screen.width - REASON_MARGIN;
    let mut rows = Vec::new();
    text::wrap(&reason, width, Wrap::Tooltip, font, &mut rows);
    let lines: Vec<String> = rows
        .iter()
        .map(|row| row.iter().map(|span| span.text.as_str()).collect())
        .collect();
    let title_width = font.str_width(title);
    let reason_height = text_lines_extent(lines.iter().map(|line| font.str_width(line))).height;
    [
        Widget::new(
            TITLE,
            GuiRect::new(
                screen.width / 2 - title_width / 2,
                TITLE_Y,
                title_width,
                LINE_HEIGHT,
            ),
            title.to_owned(),
            WidgetKind::Text {
                max_width: None,
                overflow: Overflow::Cut,
            },
        ),
        Widget::new(
            REASON,
            GuiRect::new(0, REASON_Y, screen.width, reason_height),
            lines.join("\n"),
            WidgetKind::MultiLineText {
                max_width: Some(width),
                centred: true,
            },
        ),
        Widget::new(
            CANCEL,
            GuiRect::new(
                screen.width / 2 - BUTTON_WIDTH / 2,
                BUTTON_Y,
                BUTTON_WIDTH,
                BUTTON_HEIGHT,
            ),
            language.get("gui.cancel").to_owned(),
            WidgetKind::Button,
        ),
    ]
}

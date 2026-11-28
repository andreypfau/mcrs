use mcrs_minecraft_client_widget::{
    GuiExtent, GuiRect, Overflow, Widget, WidgetId, WidgetKind, text_line_extent,
};

use mcrs_minecraft_font::ClientLanguage;
use mcrs_minecraft_font::Font;

const TEXT: WidgetId = WidgetId(0);
const TEXT_ABOVE_CENTRE: i32 = 50;

pub(crate) fn level_loading_screen(
    screen: GuiExtent,
    language: &ClientLanguage,
    font: &Font,
) -> Widget {
    let text = language.get("multiplayer.downloadingTerrain");
    let size = text_line_extent(font.str_width(text), None);
    let at = GuiRect::new(
        screen.width / 2 - size.width / 2,
        screen.height / 2 - TEXT_ABOVE_CENTRE,
        size.width,
        size.height,
    );
    Widget::new(
        TEXT,
        at,
        text.to_owned(),
        WidgetKind::Text {
            max_width: None,
            overflow: Overflow::Cut,
        },
    )
}

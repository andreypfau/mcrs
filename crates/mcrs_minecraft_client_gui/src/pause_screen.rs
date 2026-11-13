use mcrs_minecraft_client_widget::{
    Alignment, BUTTON_HEIGHT, Child, Grid, GuiExtent, GuiRect, LayoutCell, Overflow, Padding,
    RowFill, Widget, WidgetId, WidgetKind, text_line_extent,
};

use crate::font::Font;
use crate::language::ClientLanguage;

pub(crate) const TITLE: WidgetId = WidgetId(0);
pub(crate) const BACK_TO_GAME: WidgetId = WidgetId(1);
pub(crate) const OPTIONS: WidgetId = WidgetId(2);
pub(crate) const LAST: WidgetId = WidgetId(3);

const TITLE_TOP: i32 = 40;
const FIRST_ROW_PADDING_TOP: i32 = 50;
const FULL_WIDTH: i32 = 204;
const HALF_WIDTH: i32 = 98;

/// The title and the buttons of the pause screen on a screen of the size `screen`, with the
/// caption `last_key` on the last button (MENU-PAUSE-002, MENU-PAUSE-006).
pub(crate) fn pause_screen(
    screen: GuiExtent,
    language: &ClientLanguage,
    font: &Font,
    last_key: &str,
) -> Option<[Widget; 4]> {
    let title = language.get("menu.game");
    let title_size = text_line_extent(font.str_width(title), None);
    let title_at = GuiRect::new(
        screen.width / 2 - title_size.width / 2,
        TITLE_TOP,
        title_size.width,
        title_size.height,
    );
    let cell = LayoutCell {
        padding: Padding {
            left: 4,
            top: 4,
            right: 4,
            bottom: 0,
        },
        alignment: Alignment::default(),
    };
    let first = LayoutCell {
        padding: Padding {
            top: FIRST_ROW_PADDING_TOP,
            ..cell.padding
        },
        ..cell
    };
    let full = GuiExtent::new(FULL_WIDTH, BUTTON_HEIGHT);
    let half = GuiExtent::new(HALF_WIDTH, BUTTON_HEIGHT);
    let mut rows = RowFill::new(2);
    let children = [
        rows.add(Child::new(full, first), 2),
        rows.add(Child::new(half, cell), 1),
        rows.add(Child::new(full, cell), 2),
    ];
    let grid = Grid {
        children: &children,
        ..Grid::default()
    };
    let at = GuiRect::new(0, 0, screen.width, screen.height)
        .align(grid.size(), Alignment { x: 0.5, y: 0.25 });
    let mut rects = grid.rects(at.x, at.y);
    let button =
        |id, rect, key| Widget::new(id, rect, language.get(key).to_owned(), WidgetKind::Button);
    Some([
        Widget::new(
            TITLE,
            title_at,
            title.to_owned(),
            WidgetKind::Text {
                max_width: None,
                overflow: Overflow::Cut,
            },
        ),
        button(BACK_TO_GAME, rects.next()?, "menu.returnToGame"),
        button(OPTIONS, rects.next()?, "menu.options"),
        button(LAST, rects.next()?, last_key),
    ])
}

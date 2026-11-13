use mcrs_minecraft_client_options::{ClientOptions, FOV_RANGE};
use mcrs_minecraft_client_widget::{
    Alignment, BUTTON_GAP, BUTTON_HEIGHT, BUTTON_WIDTH, Child, DONE_BUTTON_WIDTH, Grid, GuiExtent,
    HeaderContentFooter, LayoutCell, Overflow, Padding, RowFill, Slider, Stack, Widget, WidgetId,
    WidgetKind, text_line_extent,
};

use crate::font::Font;
use crate::language::ClientLanguage;
use crate::option_caption::option_caption;

pub(crate) const TITLE: WidgetId = WidgetId(0);
pub(crate) const FOV: WidgetId = WidgetId(1);
pub(crate) const VIDEO: WidgetId = WidgetId(2);
pub(crate) const ACCESSIBILITY: WidgetId = WidgetId(3);
pub(crate) const DONE: WidgetId = WidgetId(4);

const HEADER_HEIGHT: i32 = 61;
const NORMAL_FOV: u8 = 70;

/// The widgets of the options screen on a screen of the size `screen`, with the FOV slider at
/// `fov` (MENU-OPT-003).
pub(crate) fn options_screen(
    screen: GuiExtent,
    language: &ClientLanguage,
    font: &Font,
    fov: u8,
) -> Option<[Widget; 5]> {
    let slider = Slider::range(
        i32::from(*FOV_RANGE.start()),
        i32::from(*FOV_RANGE.end()),
        i32::from(fov),
    )?;
    let title = language.get("options.title");
    let title_size = text_line_extent(font.str_width(title), None);
    let button = GuiExtent::new(BUTTON_WIDTH, BUTTON_HEIGHT);
    let row_children = [Child::new(button, LayoutCell::default())];
    let row = Stack::row(BUTTON_GAP, &row_children);
    let column_children = [
        Child::new(
            title_size,
            LayoutCell {
                alignment: Alignment { x: 0.5, y: 0.0 },
                ..LayoutCell::default()
            },
        ),
        Child::new(row.size(), LayoutCell::default()),
    ];
    let column = Stack::column(BUTTON_GAP, &column_children);
    let grid_cell = LayoutCell {
        padding: Padding {
            left: 4,
            top: 0,
            right: 4,
            bottom: 4,
        },
        alignment: Alignment { x: 0.5, y: 0.0 },
    };
    let mut rows = RowFill::new(2);
    let grid_children = [
        rows.add(Child::new(button, grid_cell), 1),
        rows.add(Child::new(button, grid_cell), 1),
    ];
    let grid = Grid {
        children: &grid_children,
        ..Grid::default()
    };
    let header = [Child::new(column.size(), LayoutCell::CENTER)];
    let content = [Child::new(grid.size(), LayoutCell::CENTER)];
    let footer = [Child::new(
        GuiExtent::new(DONE_BUTTON_WIDTH, BUTTON_HEIGHT),
        LayoutCell::CENTER,
    )];
    let mut parts = HeaderContentFooter {
        header_height: HEADER_HEIGHT,
        header: &header,
        content: &content,
        footer: &footer,
        ..HeaderContentFooter::default()
    }
    .rects(screen);
    let column_at = parts.next()?;
    let grid_at = parts.next()?;
    let done_at = parts.next()?;
    let mut column_rects = column.rects(column_at.x, column_at.y);
    let title_at = column_rects.next()?;
    let row_at = column_rects.next()?;
    let slider_at = row.rects(row_at.x, row_at.y).next()?;
    let mut grid_rects = grid.rects(grid_at.x, grid_at.y);
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
        Widget::new(
            FOV,
            slider_at,
            fov_caption(language, fov),
            WidgetKind::Slider(slider),
        ),
        button(VIDEO, grid_rects.next()?, "options.video"),
        button(ACCESSIBILITY, grid_rects.next()?, "options.accessibility"),
        button(DONE, done_at, "gui.done"),
    ])
}

/// The caption of the FOV slider at `fov` (MENU-VAL-001).
fn fov_caption(language: &ClientLanguage, fov: u8) -> String {
    let value = match fov {
        NORMAL_FOV => language.get("options.fov.min").to_owned(),
        max if max == *FOV_RANGE.end() => language.get("options.fov.max").to_owned(),
        other => other.to_string(),
    };
    option_caption(language, "options.generic_value", "options.fov", &value)
}

/// Stores the value of the FOV slider after its change (MENU-VAL-001).
pub(crate) fn write_option(options: &mut ClientOptions, widget: &Widget) {
    if widget.id() == FOV
        && let WidgetKind::Slider(slider) = widget.kind
        && let Some(fov) = slider.value().and_then(|fov| u8::try_from(fov).ok())
    {
        options.fov = fov;
    }
}

use mcrs_minecraft_client_widget::{
    Alignment, Child, EditBox, Grid, GridChild, GuiExtent, GuiRect, HeaderContentFooter,
    LayoutCell, Overflow, Padding, SelectionList, Stack, Widget, WidgetId, WidgetKind, WidgetValue,
    Widgets, text_line_extent,
};
use mcrs_minecraft_client_world::world_list::WorldList;
use mcrs_minecraft_save::WorldSummary;

use crate::font::{Font, Span};
use crate::language::ClientLanguage;
use crate::menu::WidgetHint;

pub(crate) const TITLE: WidgetId = WidgetId(0);
pub(crate) const SEARCH: WidgetId = WidgetId(1);
pub(crate) const LIST: WidgetId = WidgetId(2);
pub(crate) const PLAY: WidgetId = WidgetId(3);
pub(crate) const CREATE: WidgetId = WidgetId(4);
pub(crate) const DELETE: WidgetId = WidgetId(5);
pub(crate) const BACK: WidgetId = WidgetId(6);

const HEADER_HEIGHT: i32 = 49;
const FOOTER_HEIGHT: i32 = 60;
const HEADER_GAP: i32 = 4;
const SEARCH_BOX: GuiExtent = GuiExtent::new(200, 20);
const ROW_HEIGHT: i32 = 36;
const ROW_WIDTH: i32 = 270;
const COLUMN_GAP: i32 = 8;
const ROW_GAP: i32 = 4;
const WIDE: GuiExtent = GuiExtent::new(150, 20);
const NARROW: GuiExtent = GuiExtent::new(71, 20);
const HINT_COLOR: u32 = 0xFFAA_AAAA;

const ACROSS: LayoutCell = LayoutCell {
    padding: Padding {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    },
    alignment: Alignment { x: 0.5, y: 0.0 },
};

pub(crate) fn select_world_screen(
    screen: GuiExtent,
    language: &ClientLanguage,
    font: &Font,
) -> Option<([Widget; 7], WidgetHint)> {
    let title = language.get("selectWorld.title");
    let search = EditBox::default();
    let column = [
        Child::new(text_line_extent(font.str_width(title), None), ACROSS),
        Child::new(SEARCH_BOX, ACROSS),
    ];
    let column = Stack::column(HEADER_GAP, &column);
    let cell = |child, row, column, column_span| GridChild {
        child: Child::new(child, ACROSS),
        row,
        column,
        row_span: 1,
        column_span,
    };
    let grid = [
        cell(WIDE, 0, 0, 2),
        cell(WIDE, 0, 2, 2),
        cell(NARROW, 1, 0, 1),
        cell(NARROW, 1, 1, 1),
        cell(NARROW, 1, 2, 1),
        cell(NARROW, 1, 3, 1),
    ];
    let grid = Grid {
        row_gap: ROW_GAP,
        column_gap: COLUMN_GAP,
        children: &grid,
    };
    let header = [Child::new(column.size(), LayoutCell::CENTER)];
    let footer = [Child::new(grid.size(), LayoutCell::CENTER)];
    let mut parts = HeaderContentFooter {
        header_height: HEADER_HEIGHT,
        footer_height: FOOTER_HEIGHT,
        header: &header,
        footer: &footer,
        ..HeaderContentFooter::default()
    }
    .rects(screen);
    let (header, footer) = (parts.next()?, parts.next()?);
    let mut header = column.rects(header.x, header.y);
    let mut footer = grid.rects(footer.x, footer.y);
    let [play, create, _edit, delete, _recreate, back] = core::array::from_fn(|_| footer.next());
    let list = GuiRect::new(
        0,
        HEADER_HEIGHT,
        screen.width,
        screen
            .height
            .saturating_sub(HEADER_HEIGHT + FOOTER_HEIGHT)
            .max(0),
    );
    let button =
        |id, rect, key| Widget::new(id, rect, language.get(key).to_owned(), WidgetKind::Button);
    let widgets = [
        Widget::new(
            TITLE,
            header.next()?,
            title.to_owned(),
            WidgetKind::Text {
                max_width: None,
                overflow: Overflow::Cut,
            },
        ),
        Widget::new(
            SEARCH,
            header.next()?,
            language.get("selectWorld.search").to_owned(),
            WidgetKind::EditBox(search),
        ),
        Widget::new(
            LIST,
            list,
            String::new(),
            WidgetKind::SelectionList(SelectionList::new(0, ROW_HEIGHT).with_row_width(ROW_WIDTH)),
        ),
        button(PLAY, play?, "selectWorld.select"),
        button(CREATE, create?, "selectWorld.create"),
        button(DELETE, delete?, "selectWorld.delete"),
        button(BACK, back?, "gui.back"),
    ];
    let hint = WidgetHint {
        widget: SEARCH,
        text: vec![Span::new(
            language.get("gui.selectWorld.search"),
            HINT_COLOR,
        )],
    };
    Some((widgets, hint))
}

pub(crate) fn shown_worlds<'a>(
    list: &'a WorldList,
    query: &str,
) -> impl Iterator<Item = (usize, &'a WorldSummary)> {
    let worlds = match list {
        WorldList::Ready(worlds) => worlds.as_slice(),
        WorldList::Reading | WorldList::Unavailable(_) => &[],
    };
    let query = query.to_lowercase();
    worlds.iter().enumerate().filter(move |(_, world)| {
        query.is_empty()
            || world.name().to_lowercase().contains(&query)
            || world.folder_id.to_lowercase().contains(&query)
    })
}

pub(crate) fn search_query(widgets: &Widgets) -> &str {
    match widgets.value(SEARCH) {
        Some(WidgetValue::Text(text)) => text,
        _ => "",
    }
}

pub(crate) fn selected_world<'a>(
    widgets: &Widgets,
    list: &'a WorldList,
) -> Option<&'a WorldSummary> {
    world_at(widgets, list, selected_row(widgets)?)
}

pub(crate) fn selected_row(widgets: &Widgets) -> Option<usize> {
    match widgets.get(LIST).map(|widget| widget.kind()) {
        Some(WidgetKind::SelectionList(rows)) => rows.selected(),
        _ => None,
    }
}

pub(crate) fn world_at<'a>(
    widgets: &Widgets,
    list: &'a WorldList,
    row: usize,
) -> Option<&'a WorldSummary> {
    shown_worlds(list, search_query(widgets))
        .nth(row)
        .map(|(_, world)| world)
}

#[cfg(test)]
mod tests {
    mod jar {
        use mcrs_minecraft_client_widget::{GuiInput, GuiPoint};
        use time::UtcOffset;

        use super::super::*;
        use crate::create_world_screen::{NewWorldForm, create_world_menu};
        use crate::delete_world_screen::delete_world_screen;
        use crate::font::corpus_font;
        use crate::menu_test_app::{GUI, world, world_types};
        use crate::project_menu::widgets_of;
        use crate::world_entry::world_entry;
        use crate::world_list_error_screen::world_list_error_screen;

        #[test]
        #[expect(
            clippy::too_many_lines,
            reason = "one table lists the bounds of every widget of the world screens"
        )]
        fn the_world_screens_sit_where_their_rules_place_them() {
            let language = ClientLanguage::corpus();
            let font = corpus_font();
            let (select, _) = select_world_screen(GUI, language, font).unwrap();
            let delete = delete_world_screen(GUI, language, font, "Alpha").unwrap();
            let mut placed: Vec<(String, GuiRect)> = select
                .iter()
                .chain(&delete)
                .map(|widget| (widget.caption.clone(), widget.rect()))
                .collect();
            let input = GuiInput::new(GuiPoint::new(0, 0));
            let mut widgets = widgets_of(GUI, input, select).unwrap();
            widgets.set_rows(LIST, 1);
            let list = widgets.get(LIST).unwrap();
            if let WidgetKind::SelectionList(rows) = list.kind() {
                placed.extend(
                    rows.row_rect(list.rect(), 0)
                        .map(|row| (String::new(), row)),
                );
            }
            let alpha = world(
                "alpha",
                "Alpha",
                false,
                mcrs_minecraft_core::VERSION.world_version,
            );
            let east = UtcOffset::from_hms(9, 0, 0).unwrap();
            for offset in [UtcOffset::UTC, east] {
                let entry = world_entry(LIST, 0, &alpha, language, font, offset);
                placed.extend(entry.lines.iter().map(|line| {
                    let text: String = line.spans.iter().map(|span| span.text.as_str()).collect();
                    (text, GuiRect::new(line.offset.x, line.offset.y, 0, 0))
                }));
            }
            placed.extend(
                world_list_error_screen(GUI, language, font)
                    .iter()
                    .map(|widget| (widget.caption.clone(), widget.rect())),
            );
            let create = create_world_menu(
                GUI,
                input,
                language,
                font,
                &world_types(),
                &NewWorldForm::default(),
            )
            .unwrap();
            for widget in create.widgets.iter() {
                let rect = widget.rect();
                match widget.kind() {
                    WidgetKind::TabBar(bar) => {
                        placed.extend(bar.tabs().iter().enumerate().map(|(index, tab)| {
                            (tab.label.clone(), bar.tab_rect(rect, index).unwrap())
                        }))
                    }
                    WidgetKind::Text { .. } => {
                        placed.push((widget.caption.clone(), GuiRect::new(rect.x, rect.y, 0, 0)))
                    }
                    _ => placed.push((widget.caption.clone(), rect)),
                }
            }
            let expected = [
                ("Select World", GuiRect::new(182, 8, 62, 9)),
                ("search for worlds", GuiRect::new(113, 21, 200, 20)),
                ("", GuiRect::new(0, 49, 427, 131)),
                ("Play Selected World", GuiRect::new(59, 188, 150, 20)),
                ("Create New World", GuiRect::new(217, 188, 150, 20)),
                ("Delete", GuiRect::new(138, 212, 71, 20)),
                ("Back", GuiRect::new(296, 212, 71, 20)),
                (
                    "Are you sure you want to delete this world?",
                    GuiRect::new(101, 85, 224, 9),
                ),
                (
                    "'Alpha' will be lost forever! (A long time!)",
                    GuiRect::new(112, 102, 202, 9),
                ),
                ("Delete", GuiRect::new(61, 135, 150, 20)),
                ("Cancel", GuiRect::new(215, 135, 150, 20)),
                ("", GuiRect::new(78, 51, 270, 36)),
                ("Alpha", GuiRect::new(35, 1, 0, 0)),
                ("alpha (11/14/26, 3:07 PM)", GuiRect::new(35, 12, 0, 0)),
                ("Survival Mode, Version: 1.0", GuiRect::new(35, 21, 0, 0)),
                ("Alpha", GuiRect::new(35, 1, 0, 0)),
                ("alpha (11/15/26, 12:07 AM)", GuiRect::new(35, 12, 0, 0)),
                ("Survival Mode, Version: 1.0", GuiRect::new(35, 21, 0, 0)),
                ("Unable to load worlds", GuiRect::new(159, 90, 109, 9)),
                (
                    "Unable to read or access the folder where game worlds are saved!",
                    GuiRect::new(0, 110, 427, 9),
                ),
                ("Cancel", GuiRect::new(113, 140, 200, 20)),
                ("Game", GuiRect::new(28, 0, 186, 24)),
                ("World", GuiRect::new(214, 0, 186, 24)),
                ("World Name", GuiRect::new(109, 30, 0, 0)),
                ("", GuiRect::new(109, 43, 208, 20)),
                ("Game Mode: Survival", GuiRect::new(108, 71, 210, 20)),
                ("World Type: Default", GuiRect::new(58, 35, 150, 20)),
                ("Seed for the world generator", GuiRect::new(58, 63, 0, 0)),
                ("", GuiRect::new(58, 76, 308, 20)),
                ("Create New World", GuiRect::new(59, 214, 150, 20)),
                ("Cancel", GuiRect::new(217, 214, 150, 20)),
            ]
            .map(|(caption, rect)| (caption.to_owned(), rect));
            assert_eq!(placed, expected);
        }
    }
}

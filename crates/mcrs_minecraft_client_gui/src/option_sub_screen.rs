use mcrs_minecraft_client_widget::{
    AreaSlot, BUTTON_HEIGHT, BUTTON_WIDTH, Child, FOOTER_HEIGHT, GuiExtent, GuiRect, HEADER_HEIGHT,
    HeaderContentFooter, LayoutCell, Overflow, Widget, WidgetId, WidgetKind, WidgetList,
    text_line_extent,
};

use crate::font::Font;
use crate::language::ClientLanguage;

pub(crate) const TITLE: WidgetId = WidgetId(0);
pub(crate) const DONE: WidgetId = WidgetId(1);
pub(crate) const LIST: WidgetId = WidgetId(2);

pub(crate) fn option_sub_screen(
    screen: GuiExtent,
    language: &ClientLanguage,
    font: &Font,
    title_key: &str,
    done: GuiExtent,
) -> Option<[Widget; 2]> {
    let title = language.get(title_key);
    let header = [Child::new(
        text_line_extent(font.str_width(title), None),
        LayoutCell::CENTER,
    )];
    let footer = [Child::new(done, LayoutCell::CENTER)];
    let mut parts = HeaderContentFooter {
        header: &header,
        footer: &footer,
        ..HeaderContentFooter::default()
    }
    .rects(screen);
    Some([
        Widget::new(
            TITLE,
            parts.next()?,
            title.to_owned(),
            WidgetKind::Text {
                max_width: None,
                overflow: Overflow::Cut,
            },
        ),
        Widget::new(
            DONE,
            parts.next()?,
            language.get("gui.done").to_owned(),
            WidgetKind::Button,
        ),
    ])
}

pub(crate) struct OptionList {
    list: WidgetList,
    area: GuiRect,
    screen_width: i32,
}

impl OptionList {
    pub(crate) fn new(screen: GuiExtent) -> Self {
        let height = screen
            .height
            .saturating_sub(HEADER_HEIGHT.saturating_add(FOOTER_HEIGHT))
            .max(0);
        Self {
            list: WidgetList::options(),
            area: GuiRect::new(0, HEADER_HEIGHT, screen.width, height),
            screen_width: screen.width,
        }
    }

    pub(crate) fn section_label(
        &mut self,
        id: WidgetId,
        text: &str,
        font: &Font,
    ) -> Option<Widget> {
        let row = self.list.push_section_label_row();
        let rect = self.list.section_label_rect(
            self.area,
            row,
            self.screen_width,
            text_line_extent(font.str_width(text), None),
        )?;
        let label = Widget::new(id, rect, text.to_owned(), WidgetKind::SectionLabel)
            .in_area(AreaSlot { area: LIST, row });
        Some(label)
    }

    pub(crate) fn option_row<const N: usize>(
        &mut self,
        widgets: [Widget; N],
    ) -> Option<[Widget; N]> {
        let row = self.list.push_row(self.list.default_row_height());
        let rect = self.list.row_rect(self.area, row)?;
        let mut column = 0;
        Some(widgets.map(|widget| {
            let at = WidgetList::option_origin(self.screen_width, rect, column);
            column = column.saturating_add(1);
            widget
                .with_rect(GuiRect::new(at.x, at.y, BUTTON_WIDTH, BUTTON_HEIGHT))
                .in_area(AreaSlot { area: LIST, row })
        }))
    }

    pub(crate) fn into_widget(self) -> Widget {
        Widget::new(
            LIST,
            self.area,
            String::new(),
            WidgetKind::WidgetList(self.list),
        )
    }
}

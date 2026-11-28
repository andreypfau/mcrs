use crate::cycle_button::CycleButton;
use crate::edit_box::EditBox;
use crate::gui_extent::GuiExtent;
use crate::gui_rect::GuiRect;
use crate::icon_form::IconForm;
use crate::overflow::Overflow;
use crate::scroll_area::ScrollArea;
use crate::scroll_panel::ScrollPanel;
use crate::selection_list::SelectionList;
use crate::slider::Slider;
use crate::tab_bar::TabBar;
use crate::widget_list::WidgetList;

#[non_exhaustive]
#[derive(Clone, Debug)]
pub enum WidgetKind {
    Button,
    IconButton {
        icon: GuiExtent,
        form: IconForm,
    },
    ImageButton,
    Checkbox {
        selected: bool,
    },
    CycleButton(CycleButton),
    Slider(Slider),
    Text {
        max_width: Option<i32>,
        overflow: Overflow,
    },
    MultiLineText {
        max_width: Option<i32>,
        centred: bool,
    },
    SelectionList(SelectionList),
    WidgetList(WidgetList),
    ScrollPanel(ScrollPanel),
    SectionLabel,
    TabBar(TabBar),
    EditBox(EditBox),
}

impl WidgetKind {
    pub(crate) const fn is_text(&self) -> bool {
        matches!(self, Self::Text { .. } | Self::MultiLineText { .. })
    }

    #[must_use]
    pub const fn is_scroll_container(&self) -> bool {
        matches!(
            self,
            Self::SelectionList(_) | Self::WidgetList(_) | Self::ScrollPanel(_)
        )
    }

    pub(crate) fn scroll_area(&self, rect: GuiRect) -> Option<ScrollArea> {
        match self {
            Self::SelectionList(list) => Some(list.scroll_area(rect)),
            Self::WidgetList(list) => Some(list.scroll_area(rect)),
            Self::ScrollPanel(panel) => Some(panel.scroll_area(rect)),
            _ => None,
        }
    }

    pub(crate) fn offset_mut(&mut self) -> Option<&mut f64> {
        match self {
            Self::SelectionList(list) => Some(&mut list.offset),
            Self::WidgetList(list) => Some(&mut list.offset),
            Self::ScrollPanel(panel) => Some(&mut panel.offset),
            _ => None,
        }
    }
}

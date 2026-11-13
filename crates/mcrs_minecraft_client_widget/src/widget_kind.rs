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

/// What a widget is, with the state that only its kind has.
#[non_exhaustive]
#[derive(Clone, Debug)]
pub enum WidgetKind {
    /// A button (WID-BTN-001 to WID-BTN-005).
    Button,
    /// A button with an icon sprite of `icon` GUI units (WID-ICON-001).
    IconButton { icon: GuiExtent, form: IconForm },
    /// A button that the screen draws with one sprite for each state and no caption
    /// (WID-ICON-002).
    ImageButton,
    /// A checkbox (WID-CHK-001 to WID-CHK-003).
    Checkbox { selected: bool },
    /// A button that selects the next or the previous of its values (WID-CYC-001 to
    /// WID-CYC-003).
    CycleButton(CycleButton),
    /// A slider (WID-SLD-001 to WID-SLD-007).
    Slider(Slider),
    /// A one-line text widget, never active (WID-TXT-001).
    Text {
        max_width: Option<i32>,
        overflow: Overflow,
    },
    /// A multi-line text widget, never active; the screen wraps and cuts its lines
    /// (WID-TXT-002).
    MultiLineText {
        max_width: Option<i32>,
        centred: bool,
    },
    /// A list whose rows the player selects (LIST-SEL-001 to LIST-SEL-006).
    SelectionList(SelectionList),
    /// A list whose rows hold widgets of the screen (LIST-WID-001, LIST-WID-002).
    WidgetList(WidgetList),
    /// A scroll area over a layout of widgets of the screen (SCRL-PANEL-001, SCRL-PANEL-002).
    ScrollPanel(ScrollPanel),
    /// The label of a section label row of an option list (LIST-OPT-002).
    SectionLabel,
    /// A row of tabs with one current tab (TAB-BAR-001 to TAB-FOCUS-001).
    TabBar(TabBar),
    /// A one-line edit box (EDIT-OPT-001 to EDIT-DEV-002).
    EditBox(EditBox),
}

impl WidgetKind {
    pub(crate) const fn is_text(&self) -> bool {
        matches!(self, Self::Text { .. } | Self::MultiLineText { .. })
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

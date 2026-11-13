use alloc::vec::Vec;

use crate::gui_extent::GuiExtent;
use crate::gui_input::GuiInput;
use crate::gui_rect::GuiRect;
use crate::key_modifiers::KeyModifiers;
use crate::pointer_button::PointerButton;
use crate::scroll_area::ScrollArea;
use crate::widget::Widget;
use crate::widget_event::WidgetEvent;
use crate::widget_id::WidgetId;
use crate::widget_id_taken::WidgetIdTaken;
use crate::widget_kind::WidgetKind;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Drag {
    None,
    Widget,
    Scrollbar { area: usize, last_y: f64 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Press {
    pub(crate) button: PointerButton,
    pub(crate) at_ms: u64,
}

/// The widget list or the scroll panel that holds a widget.
pub(crate) struct Member<'a> {
    pub(crate) index: usize,
    pub(crate) widget: &'a Widget,
    pub(crate) area: ScrollArea,
    pub(crate) row: usize,
}

/// The widgets of one screen in their order of addition, which is the order of the pointer and
/// of the draw, with the focus, the drag and the input of the GUI.
#[derive(Clone, Debug)]
pub struct Widgets {
    pub(crate) widgets: Vec<Widget>,
    pub(crate) screen: GuiExtent,
    pub(crate) input: GuiInput,
    pub(crate) focused: Option<usize>,
    pub(crate) drag: Drag,
    pub(crate) change_mode: bool,
    pub(crate) last_press: Option<Press>,
}

impl Widgets {
    /// A screen of the size `screen` with no widget, which takes `input` from the client.
    #[must_use]
    pub const fn new(screen: GuiExtent, input: GuiInput) -> Self {
        Self {
            widgets: Vec::new(),
            screen,
            input,
            focused: None,
            drag: Drag::None,
            change_mode: false,
            last_press: None,
        }
    }

    /// Adds `widget` after the others; fails when the screen already has its id.
    pub fn push(&mut self, widget: Widget) -> Result<(), WidgetIdTaken> {
        if self.index(widget.id()).is_some() {
            return Err(WidgetIdTaken(widget.id()));
        }
        self.widgets.push(widget);
        Ok(())
    }

    /// Removes every widget and clears the focus and the drag, for a screen that builds its
    /// widgets again (WID-SCR-003); the input stays.
    pub fn clear(&mut self) {
        self.widgets.clear();
        self.focused = None;
        self.drag = Drag::None;
        self.change_mode = false;
    }

    /// The widget with the id `id`.
    #[must_use]
    pub fn get(&self, id: WidgetId) -> Option<&Widget> {
        self.widgets.iter().find(|widget| widget.id() == id)
    }

    /// The widget with the id `id`, to change its bounds, caption or states.
    pub fn get_mut(&mut self, id: WidgetId) -> Option<&mut Widget> {
        self.widgets.iter_mut().find(|widget| widget.id() == id)
    }

    /// The widgets in their order of addition, which is the order of the draw.
    pub fn iter(&self) -> impl Iterator<Item = &Widget> {
        self.widgets.iter()
    }

    /// The size of the screen, W × H.
    #[must_use]
    pub const fn screen(&self) -> GuiExtent {
        self.screen
    }

    /// Takes a new screen size for a re-arrange that keeps the widgets, their values and the
    /// focus (WID-SCR-003); the screen then sets the new bounds of each widget.
    pub fn set_screen(&mut self, screen: GuiExtent) {
        self.screen = screen;
    }

    /// Gives the widget `id` the bounds `rect`; a selection list then scrolls to its selected row,
    /// and a scroll area clamps its scroll offset (LIST-GEO-001, SCRL-AREA-004).
    pub fn set_rect(&mut self, id: WidgetId, rect: GuiRect) {
        let keyboard = self.input.device().is_keyboard();
        let Some(widget) = self.get_mut(id) else {
            return;
        };
        widget.rect = rect;
        if let WidgetKind::SelectionList(list) = &mut widget.kind
            && let Some(row) = list.selected
        {
            list.reveal(rect, row, keyboard);
        }
        refit(widget);
    }

    /// Replaces the rows of the selection list `id` with `rows` rows, with no selected row
    /// (LIST-SEL-006).
    pub fn set_rows(&mut self, id: WidgetId, rows: usize) {
        let Some(widget) = self.get_mut(id) else {
            return;
        };
        if let WidgetKind::SelectionList(list) = &mut widget.kind {
            list.rows = rows;
            list.selected = None;
        }
        refit(widget);
    }

    /// Adds a row at `index` to the selection list `id`; a row added above the first row keeps
    /// the distance of the view from the bottom of the content (SCRL-AREA-004).
    pub fn insert_row(&mut self, id: WidgetId, index: usize) {
        let Some(widget) = self.get_mut(id) else {
            return;
        };
        let rect = widget.rect;
        if let WidgetKind::SelectionList(list) = &mut widget.kind {
            list.insert_row(rect, index);
        }
        refit(widget);
    }

    /// Removes the row `index` of the selection list `id`, and its selection when it is selected
    /// (LIST-SEL-006, SCRL-AREA-004).
    pub fn remove_row(&mut self, id: WidgetId, index: usize) {
        let Some(widget) = self.get_mut(id) else {
            return;
        };
        let rect = widget.rect;
        if let WidgetKind::SelectionList(list) = &mut widget.kind {
            list.remove_row(rect, index);
        }
        refit(widget);
    }

    /// Sets the scroll offset of the scroll area `id` to `offset`, clamped to its limits
    /// (SCRL-AREA-001).
    pub fn set_scroll_offset(&mut self, id: WidgetId, offset: f64) {
        let Some(widget) = self.get_mut(id) else {
            return;
        };
        if let Some(area) = widget.kind.scroll_area(widget.rect) {
            set_offset(widget, area.clamp(offset));
        }
    }

    /// The input of the GUI, for the next screen to take.
    #[must_use]
    pub const fn input(&self) -> GuiInput {
        self.input
    }

    /// Stores the value of each deferred slider in the first frame at least 600 ms after its
    /// last change; call it once in each frame (WID-SLD-007).
    pub fn apply_deferred(&mut self, now_ms: u64, events: &mut Vec<WidgetEvent>) {
        for widget in &mut self.widgets {
            let id = widget.id();
            if let WidgetKind::Slider(slider) = &mut widget.kind
                && slider.apply_due(now_ms)
            {
                events.push(WidgetEvent::ValueChanged { widget: id });
            }
        }
    }

    pub(crate) fn index(&self, id: WidgetId) -> Option<usize> {
        self.widgets.iter().position(|widget| widget.id() == id)
    }

    pub(crate) fn is_focused(&self, widget: &Widget) -> bool {
        self.focused() == Some(widget.id())
    }

    pub(crate) fn member(&self, widget: &Widget) -> Option<Member<'_>> {
        let slot = widget.area?;
        let index = self.index(slot.area)?;
        let area_widget = self.widgets.get(index)?;
        let row = match area_widget.kind {
            WidgetKind::WidgetList(_) => slot.row,
            WidgetKind::ScrollPanel(_) => 0,
            _ => return None,
        };
        Some(Member {
            index,
            widget: area_widget,
            area: area_widget.kind.scroll_area(area_widget.rect)?,
            row,
        })
    }

    pub(crate) fn members(&self, area: usize) -> impl Iterator<Item = usize> + Clone + '_ {
        let id = self.widgets.get(area).map(Widget::id);
        self.widgets
            .iter()
            .enumerate()
            .filter(move |(_, widget)| widget.area.is_some_and(|slot| Some(slot.area) == id))
            .map(|(index, _)| index)
    }

    pub(crate) fn row_members(
        &self,
        list: usize,
        row: usize,
    ) -> impl Iterator<Item = usize> + Clone + '_ {
        self.members(list).filter(move |index| {
            self.widgets
                .get(*index)
                .and_then(|widget| widget.area)
                .is_some_and(|slot| slot.row == row)
        })
    }

    pub(crate) fn list_rows(&self, list: usize) -> usize {
        match self.widgets.get(list).map(|widget| &widget.kind) {
            Some(WidgetKind::WidgetList(list)) => list.rows(),
            _ => 0,
        }
    }

    pub(crate) fn accepts(&self, index: usize) -> bool {
        let Some(widget) = self.widgets.get(index) else {
            return false;
        };
        widget.takes_input()
            && self.member(widget).is_none_or(|member| {
                member.widget.takes_input()
                    && match &member.widget.kind {
                        WidgetKind::WidgetList(list) => member.row < list.rows(),
                        _ => true,
                    }
            })
    }
}

pub(crate) fn set_offset(widget: &mut Widget, offset: f64) {
    if let Some(stored) = widget.kind.offset_mut() {
        *stored = offset;
    }
}

fn refit(widget: &mut Widget) {
    if let Some(area) = widget.kind.scroll_area(widget.rect) {
        set_offset(widget, area.offset());
    }
}

pub(crate) fn activate(
    widget: &mut Widget,
    modifiers: KeyModifiers,
    events: &mut Vec<WidgetEvent>,
) -> bool {
    let id = widget.id();
    match &mut widget.kind {
        WidgetKind::Button | WidgetKind::IconButton { .. } | WidgetKind::ImageButton => {
            events.push(WidgetEvent::Activated {
                widget: id,
                modifiers,
            });
        }
        WidgetKind::Checkbox { selected } => {
            *selected = !*selected;
            events.push(WidgetEvent::ValueChanged { widget: id });
        }
        WidgetKind::CycleButton(cycle) => {
            if modifiers.shift {
                cycle.select_previous();
            } else {
                cycle.select_next();
            }
            events.push(WidgetEvent::ValueChanged { widget: id });
        }
        WidgetKind::Slider(_)
        | WidgetKind::Text { .. }
        | WidgetKind::MultiLineText { .. }
        | WidgetKind::SelectionList(_)
        | WidgetKind::WidgetList(_)
        | WidgetKind::ScrollPanel(_)
        | WidgetKind::SectionLabel
        | WidgetKind::TabBar(_)
        | WidgetKind::EditBox(_) => {
            return false;
        }
    }
    true
}

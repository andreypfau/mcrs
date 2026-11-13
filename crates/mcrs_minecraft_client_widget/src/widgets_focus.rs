use alloc::vec::Vec;

use crate::gui_key::GuiKey;
use crate::gui_rect::GuiRect;
use crate::input_device::InputDevice;
use crate::key_modifiers::KeyModifiers;
use crate::navigation::{self, ArrowDirection, TabDirection, TabKey};
use crate::slider::SliderStep;
use crate::widget::Widget;
use crate::widget_event::WidgetEvent;
use crate::widget_id::WidgetId;
use crate::widget_kind::WidgetKind;
use crate::widget_placement::WidgetPlacement;
use crate::widgets::{Widgets, activate, set_offset};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FocusEntry {
    Edit,
    Browse,
}

impl Widgets {
    /// The focused widget (WID-FOC-001).
    #[must_use]
    pub fn focused(&self) -> Option<WidgetId> {
        self.focused
            .and_then(|index| self.widgets.get(index))
            .map(Widget::id)
    }

    /// Sets the initial focus of a screen that built its widgets: `initial` when it can take the
    /// focus, else the first widget for Tab after a key, else none (WID-FOC-002); an edit box
    /// that gets it at `now_ms` enters edit mode (EDIT-FOCUS-001).
    pub fn focus_initial(
        &mut self,
        initial: Option<WidgetId>,
        now_ms: u64,
        events: &mut Vec<WidgetEvent>,
    ) {
        let before = self.focused;
        let named = initial
            .and_then(|id| self.index(id))
            .filter(|index| self.widgets.get(*index).is_some_and(Widget::takes_input));
        let target = named.or_else(|| {
            if self.input.device().is_keyboard() {
                self.tab_target(None, TabDirection::Forward)
            } else {
                None
            }
        });
        self.focus(target, events);
        self.settle_focus(before, now_ms, FocusEntry::Edit);
    }

    /// A key press at `now_ms` while the screen is open; `width` is the text width of the font
    /// (WID-IN-001, WID-FOC-003, WID-SLD-003, LIST-SEL-004, LIST-SEL-005, LIST-WID-002,
    /// TAB-KEY-001, TAB-FOCUS-001, EDIT-KEY-001, EDIT-FOCUS-001, EDIT-FOCUS-002).
    pub fn key_press(
        &mut self,
        key: GuiKey,
        modifiers: KeyModifiers,
        now_ms: u64,
        width: &impl Fn(&str) -> i32,
        events: &mut Vec<WidgetEvent>,
    ) {
        let (device, arrow) = match key {
            GuiKey::Tab => (Some(InputDevice::Tab), None),
            GuiKey::Up => (Some(InputDevice::Arrow), Some(ArrowDirection::Up)),
            GuiKey::Down => (Some(InputDevice::Arrow), Some(ArrowDirection::Down)),
            GuiKey::Left => (Some(InputDevice::Arrow), Some(ArrowDirection::Left)),
            GuiKey::Right => (Some(InputDevice::Arrow), Some(ArrowDirection::Right)),
            GuiKey::Enter | GuiKey::Space | GuiKey::NumpadEnter => {
                (Some(InputDevice::Activation), None)
            }
            GuiKey::Backspace | GuiKey::Delete | GuiKey::Home | GuiKey::End | GuiKey::Digit(_) => {
                (None, None)
            }
        };
        let text_input = self.captures_keyboard() && device != Some(InputDevice::Tab);
        if let Some(device) = device.filter(|_| !text_input) {
            self.input.set_device(device);
        }
        let before = self.focused;
        let entry = if key.is_arrow() {
            FocusEntry::Browse
        } else {
            FocusEntry::Edit
        };
        if !self.tab_bar_uses(key, modifiers, events)
            && !self.focused_widget_uses(key, modifiers, now_ms, width, events)
        {
            self.navigate(key, modifiers, arrow, events);
        }
        self.settle_focus(before, now_ms, entry);
    }

    fn navigate(
        &mut self,
        key: GuiKey,
        modifiers: KeyModifiers,
        arrow: Option<ArrowDirection>,
        events: &mut Vec<WidgetEvent>,
    ) {
        if key == GuiKey::Tab {
            let direction = if modifiers.shift {
                TabDirection::Backward
            } else {
                TabDirection::Forward
            };
            self.tab(direction, events);
        } else if let Some(direction) = arrow {
            self.arrow(direction, events);
        }
    }

    fn tab_key(&self, index: usize, widget: &Widget) -> TabKey {
        match self.member(widget) {
            Some(member) => TabKey {
                group: member.widget.tab_group,
                owner: member.index,
                row: member.row.saturating_add(1),
                index,
            },
            None => TabKey {
                group: widget.tab_group,
                owner: index,
                row: 0,
                index: 0,
            },
        }
    }

    fn is_tab_stop(&self, index: usize, widget: &Widget) -> bool {
        self.accepts(index)
            && (self.member(widget).is_some()
                || match &widget.kind {
                    WidgetKind::WidgetList(_) | WidgetKind::ScrollPanel(_) => false,
                    WidgetKind::SelectionList(list) => list.rows() > 0,
                    _ => true,
                })
    }

    fn tab_target(&self, current: Option<TabKey>, direction: TabDirection) -> Option<usize> {
        let stops = self
            .widgets
            .iter()
            .enumerate()
            .filter(|(index, widget)| self.is_tab_stop(*index, widget))
            .map(|(index, widget)| (self.tab_key(index, widget), index));
        navigation::tab_target(stops, current, direction)
    }

    fn tab(&mut self, direction: TabDirection, events: &mut Vec<WidgetEvent>) {
        let current = self
            .focused
            .and_then(|index| Some(self.tab_key(index, self.widgets.get(index)?)));
        match self.tab_target(current, direction) {
            Some(target) => {
                let entry = match direction {
                    TabDirection::Forward => ArrowDirection::Down,
                    TabDirection::Backward => ArrowDirection::Up,
                };
                self.enter(target, entry, events);
            }
            None => self.focus(None, events),
        }
    }

    fn arrow(&mut self, direction: ArrowDirection, events: &mut Vec<WidgetEvent>) {
        let Some(from) = self.focused else {
            self.screen_arrow(None, None, direction, events);
            return;
        };
        let Some(widget) = self.widgets.get(from) else {
            return;
        };
        if let Some(member) = self.member(widget)
            && matches!(member.widget.kind, WidgetKind::WidgetList(_))
        {
            let (list, row) = (member.index, member.row);
            self.member_arrow(from, list, row, direction, events);
            return;
        }
        let rect = match self.placement(widget) {
            WidgetPlacement::Area { shift, .. } => widget.rect.shifted_up(shift),
            WidgetPlacement::Screen | WidgetPlacement::Hidden => widget.rect,
        };
        match &widget.kind {
            WidgetKind::SelectionList(list) => {
                let rows = list.rows();
                let next = match (direction, list.selected()) {
                    (ArrowDirection::Down, Some(row)) => {
                        row.checked_add(1).filter(|next| *next < rows)
                    }
                    (ArrowDirection::Down, None) => (rows > 0).then_some(0),
                    (ArrowDirection::Up, Some(row)) => row.checked_sub(1),
                    (ArrowDirection::Up, None) => rows.checked_sub(1),
                    (ArrowDirection::Left | ArrowDirection::Right, _) => None,
                };
                self.select(from, next, events);
                if next.is_none() {
                    self.screen_arrow(Some(rect), Some(from), direction, events);
                }
            }
            WidgetKind::WidgetList(_) => match self.list_entry(from, direction) {
                Some(member) => self.focus(Some(member), events),
                None => self.screen_arrow(Some(rect), Some(from), direction, events),
            },
            WidgetKind::TabBar(bar) => {
                let side = match direction {
                    ArrowDirection::Left => bar.beside(TabDirection::Backward),
                    ArrowDirection::Right => bar.beside(TabDirection::Forward),
                    ArrowDirection::Up | ArrowDirection::Down => None,
                };
                match side {
                    Some(tab) => self.set_tab(from, tab, events),
                    None => self.screen_arrow(Some(rect), Some(from), direction, events),
                }
            }
            _ => self.screen_arrow(Some(rect), Some(from), direction, events),
        }
    }

    fn member_arrow(
        &mut self,
        from: usize,
        list: usize,
        row: usize,
        direction: ArrowDirection,
        events: &mut Vec<WidgetEvent>,
    ) {
        let column = self
            .row_members(list, row)
            .position(|index| index == from)
            .unwrap_or(0);
        let accepts = |index: &usize| self.accepts(*index);
        let target = match direction {
            ArrowDirection::Right => self
                .row_members(list, row)
                .skip(column.saturating_add(1))
                .find(accepts),
            ArrowDirection::Left => self
                .row_members(list, row)
                .take(column)
                .filter(accepts)
                .last(),
            ArrowDirection::Up | ArrowDirection::Down => {
                self.row_after(list, row, column, direction)
            }
        };
        let target = target.or_else(|| {
            if matches!(direction, ArrowDirection::Up | ArrowDirection::Down) {
                return None;
            }
            let from_rect = self.widgets.get(from)?.rect;
            let candidates = self
                .row_members(list, row)
                .filter(move |index| *index != from)
                .filter(accepts)
                .filter_map(|index| Some((index, self.widgets.get(index)?.rect)));
            navigation::arrow_target(candidates, Some(from_rect), self.screen, direction)
        });
        match target {
            Some(target) => self.focus(Some(target), events),
            None => {
                let bounds = self.widgets.get(list).map(|widget| widget.rect);
                self.screen_arrow(bounds, Some(list), direction, events);
            }
        }
    }

    fn list_entry(&self, list: usize, direction: ArrowDirection) -> Option<usize> {
        let rows = self.list_rows(list);
        let filled = |row: &usize| self.row_members(list, *row).next().is_some();
        let row = if direction == ArrowDirection::Up {
            (0..rows).rev().find(filled)?
        } else {
            (0..rows).find(filled)?
        };
        let (column, target) = if direction == ArrowDirection::Left {
            self.row_members(list, row).enumerate().last()?
        } else {
            (0, self.row_members(list, row).next()?)
        };
        if self.accepts(target) {
            return Some(target);
        }
        let onward = if direction == ArrowDirection::Up {
            ArrowDirection::Up
        } else {
            ArrowDirection::Down
        };
        self.row_after(list, row, column, onward)
    }

    fn row_after(
        &self,
        list: usize,
        mut row: usize,
        column: usize,
        direction: ArrowDirection,
    ) -> Option<usize> {
        let rows = self.list_rows(list);
        loop {
            row = if direction == ArrowDirection::Up {
                row.checked_sub(1)?
            } else {
                row.checked_add(1).filter(|next| *next < rows)?
            };
            let Some(last) = self.row_members(list, row).count().checked_sub(1) else {
                continue;
            };
            let target = self.row_members(list, row).nth(column.min(last))?;
            if self.accepts(target) {
                return Some(target);
            }
        }
    }

    fn screen_arrow(
        &mut self,
        from: Option<GuiRect>,
        exclude: Option<usize>,
        direction: ArrowDirection,
        events: &mut Vec<WidgetEvent>,
    ) {
        let candidates = self
            .widgets
            .iter()
            .enumerate()
            .filter(move |(index, _)| Some(*index) != exclude)
            .filter_map(|(index, widget)| self.screen_candidate(index, widget, direction));
        if let Some(target) = navigation::arrow_target(candidates, from, self.screen, direction) {
            self.enter(target, direction, events);
        }
    }

    fn screen_candidate(
        &self,
        index: usize,
        widget: &Widget,
        direction: ArrowDirection,
    ) -> Option<(usize, GuiRect)> {
        if !self.accepts(index) {
            return None;
        }
        if let Some(member) = self.member(widget) {
            return matches!(member.widget.kind, WidgetKind::ScrollPanel(_))
                .then(|| (index, widget.rect.shifted_up(member.area.shift())));
        }
        let candidate = match &widget.kind {
            WidgetKind::WidgetList(_) => self.list_entry(index, direction).is_some(),
            WidgetKind::ScrollPanel(_) => false,
            WidgetKind::SelectionList(list) => list.rows() > 0,
            _ => true,
        };
        candidate.then_some((index, widget.rect))
    }

    fn enter(&mut self, target: usize, direction: ArrowDirection, events: &mut Vec<WidgetEvent>) {
        match self.widgets.get(target).map(|widget| &widget.kind) {
            Some(WidgetKind::SelectionList(list)) => {
                let last = list.rows().checked_sub(1);
                let row = list.selected().or(if direction == ArrowDirection::Up {
                    last
                } else {
                    last.map(|_| 0)
                });
                self.focus(Some(target), events);
                self.select(target, row, events);
            }
            Some(WidgetKind::WidgetList(_)) => {
                let member = self.list_entry(target, direction);
                self.focus(Some(member.unwrap_or(target)), events);
            }
            _ => self.focus(Some(target), events),
        }
    }

    pub(crate) fn select(
        &mut self,
        index: usize,
        row: Option<usize>,
        events: &mut Vec<WidgetEvent>,
    ) {
        let keyboard = self.input.device().is_keyboard();
        let Some(widget) = self.widgets.get_mut(index) else {
            return;
        };
        let (id, rect) = (widget.id(), widget.rect);
        let WidgetKind::SelectionList(list) = &mut widget.kind else {
            return;
        };
        if list.selected != row {
            list.selected = row;
            events.push(WidgetEvent::SelectionChanged { widget: id, row });
        }
        if let Some(row) = row {
            list.reveal(rect, row, keyboard);
        }
    }

    fn reveal(&mut self, index: usize) {
        let keyboard = self.input.device().is_keyboard();
        let Some(widget) = self.widgets.get(index) else {
            return;
        };
        let Some(member) = self.member(widget) else {
            return;
        };
        let offset = match &member.widget.kind {
            WidgetKind::WidgetList(list) => list
                .row_rect(member.area.rect, member.row)
                .map(|row| member.area.revealed_row(row.y, row.bottom(), keyboard)),
            _ => Some(
                member
                    .area
                    .revealed_widget(widget.rect.y, widget.rect.bottom(), keyboard),
            ),
        };
        let area = member.index;
        if let Some(offset) = offset
            && let Some(area) = self.widgets.get_mut(area)
        {
            set_offset(area, offset);
        }
    }

    pub(crate) fn focus(&mut self, index: Option<usize>, events: &mut Vec<WidgetEvent>) {
        if let Some(index) = index {
            self.reveal(index);
        }
        let keeps_focus = self
            .focused
            .and_then(|focused| self.edit_box(focused))
            .is_some_and(|edit| !edit.can_lose_focus);
        if index == self.focused || (index.is_none() && keeps_focus) {
            return;
        }
        self.focused = index;
        let slider = index
            .and_then(|index| self.widgets.get(index))
            .is_some_and(|widget| matches!(widget.kind, WidgetKind::Slider(_)));
        self.change_mode =
            slider && matches!(self.input.device(), InputDevice::Mouse | InputDevice::Tab);
        events.push(WidgetEvent::FocusChanged {
            focused: self.focused(),
        });
    }

    fn tab_bar_uses(
        &mut self,
        key: GuiKey,
        modifiers: KeyModifiers,
        events: &mut Vec<WidgetEvent>,
    ) -> bool {
        if !modifiers.control {
            return false;
        }
        let Some((index, bar)) =
            self.widgets
                .iter()
                .enumerate()
                .find_map(|(index, widget)| match &widget.kind {
                    WidgetKind::TabBar(bar) if widget.visible => Some((index, bar)),
                    _ => None,
                })
        else {
            return false;
        };
        let target = match key {
            GuiKey::Digit(digit) if digit <= 9 => bar.digit_target(digit),
            GuiKey::Tab if modifiers.shift => bar.cycled(TabDirection::Backward),
            GuiKey::Tab => bar.cycled(TabDirection::Forward),
            _ => return false,
        };
        if let Some(tab) = target {
            self.set_tab(index, tab, events);
        }
        true
    }

    pub(crate) fn set_tab(&mut self, index: usize, tab: usize, events: &mut Vec<WidgetEvent>) {
        let Some(widget) = self.widgets.get_mut(index) else {
            return;
        };
        let id = widget.id();
        if let WidgetKind::TabBar(bar) = &mut widget.kind
            && bar.current != tab
        {
            bar.current = tab;
            events.push(WidgetEvent::TabChanged { widget: id, tab });
        }
    }

    fn focused_widget_uses(
        &mut self,
        key: GuiKey,
        modifiers: KeyModifiers,
        now_ms: u64,
        width: &impl Fn(&str) -> i32,
        events: &mut Vec<WidgetEvent>,
    ) -> bool {
        let Some(widget) = self.focused.and_then(|index| self.widgets.get_mut(index)) else {
            return false;
        };
        if !widget.takes_input() {
            return false;
        }
        let (id, rect) = (widget.id(), widget.rect);
        if let WidgetKind::SelectionList(list) = &widget.kind {
            let row = list
                .selected()
                .filter(|_| matches!(key, GuiKey::Enter | GuiKey::NumpadEnter));
            if let Some(row) = row {
                events.push(WidgetEvent::RowActivated { widget: id, row });
            }
            return row.is_some();
        }
        if let WidgetKind::Slider(slider) = &mut widget.kind {
            if key.is_activation() {
                self.change_mode = !self.change_mode;
                return true;
            }
            let step = match key {
                GuiKey::Left => SliderStep::Down,
                GuiKey::Right => SliderStep::Up,
                _ => return false,
            };
            if self.change_mode {
                events.extend(slider.step(id, step, rect, now_ms));
            }
            return self.change_mode;
        }
        if let WidgetKind::EditBox(edit) = &mut widget.kind {
            let Some(changed) = edit.key_press(key, modifiers) else {
                return false;
            };
            edit.scroll(rect, width);
            if changed {
                events.push(WidgetEvent::ValueChanged { widget: id });
            }
            return true;
        }
        key.is_activation() && activate(widget, modifiers, events)
    }

    pub(crate) fn settle_focus(&mut self, before: Option<usize>, now_ms: u64, entry: FocusEntry) {
        if before == self.focused {
            return;
        }
        if let Some(edit) = before.and_then(|index| self.edit_box_mut(index))
            && edit.can_lose_focus
        {
            edit.editing = false;
        }
        if let Some(edit) = self.focused.and_then(|index| self.edit_box_mut(index)) {
            edit.focus_ms = now_ms;
            edit.editing = entry == FocusEntry::Edit;
        }
    }
}

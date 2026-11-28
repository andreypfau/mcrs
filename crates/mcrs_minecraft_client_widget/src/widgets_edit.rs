use alloc::borrow::ToOwned;
use alloc::string::String;
use alloc::vec::Vec;

use crate::edit_box::EditBox;
use crate::gui_rect::GuiRect;
use crate::widget_event::WidgetEvent;
use crate::widget_id::WidgetId;
use crate::widget_kind::WidgetKind;
use crate::widgets::Widgets;

struct EditTarget<'a> {
    id: WidgetId,
    rect: GuiRect,
    edit: &'a mut EditBox,
}

impl Widgets {
    #[must_use]
    pub fn captures_keyboard(&self) -> bool {
        self.editing_box().is_some_and(EditBox::is_editable)
    }

    pub fn type_text(
        &mut self,
        text: &str,
        width: &impl Fn(&str) -> i32,
        events: &mut Vec<WidgetEvent>,
    ) -> bool {
        if !self.captures_keyboard() {
            return false;
        }
        let Some(EditTarget { id, rect, edit }) = self.editing_box_mut() else {
            return false;
        };
        let mut changed = false;
        let mut buffer = [0_u8; 4];
        for c in text.chars() {
            changed |= edit.paste(c.encode_utf8(&mut buffer), width);
            edit.scroll(rect, width);
        }
        if changed {
            events.push(WidgetEvent::ValueChanged { widget: id });
        }
        true
    }

    pub fn paste(
        &mut self,
        text: &str,
        width: &impl Fn(&str) -> i32,
        events: &mut Vec<WidgetEvent>,
    ) {
        let Some(EditTarget { id, rect, edit }) = self.editing_box_mut() else {
            return;
        };
        if edit.paste(text, width) {
            events.push(WidgetEvent::ValueChanged { widget: id });
        }
        edit.scroll(rect, width);
    }

    #[must_use]
    pub fn copy(&self) -> Option<&str> {
        self.editing_box()
            .map(EditBox::selected)
            .filter(|selected| !selected.is_empty())
    }

    pub fn cut(
        &mut self,
        width: &impl Fn(&str) -> i32,
        events: &mut Vec<WidgetEvent>,
    ) -> Option<String> {
        let EditTarget { id, rect, edit } = self.editing_box_mut()?;
        let selected = edit.selected();
        if selected.is_empty() {
            return None;
        }
        let selected = selected.to_owned();
        if edit.cut() {
            events.push(WidgetEvent::ValueChanged { widget: id });
        }
        edit.scroll(rect, width);
        Some(selected)
    }

    pub fn select_all(&mut self) {
        if let Some(target) = self.editing_box_mut() {
            target.edit.select_all();
        }
    }

    pub fn change_edit_box(
        &mut self,
        id: WidgetId,
        width: &impl Fn(&str) -> i32,
        events: &mut Vec<WidgetEvent>,
        change: impl FnOnce(&mut EditBox) -> bool,
    ) {
        let Some(widget) = self.get_mut(id) else {
            return;
        };
        let rect = widget.rect;
        let WidgetKind::EditBox(edit) = &mut widget.kind else {
            return;
        };
        let changed = change(edit);
        edit.scroll(rect, width);
        if changed {
            events.push(WidgetEvent::ValueChanged { widget: id });
        }
    }

    pub(crate) fn edit_box(&self, index: usize) -> Option<&EditBox> {
        match &self.widgets.get(index)?.kind {
            WidgetKind::EditBox(edit) => Some(edit),
            _ => None,
        }
    }

    fn editing_box(&self) -> Option<&EditBox> {
        let index = self.focused?;
        let edit = self.edit_box(index)?;
        let widget = self.widgets.get(index)?;
        (widget.takes_input() && edit.is_editing()).then_some(edit)
    }

    fn editing_box_mut(&mut self) -> Option<EditTarget<'_>> {
        let widget = self.widgets.get_mut(self.focused?)?;
        if !widget.takes_input() {
            return None;
        }
        let (id, rect) = (widget.id(), widget.rect);
        match &mut widget.kind {
            WidgetKind::EditBox(edit) if edit.is_editing() => Some(EditTarget { id, rect, edit }),
            _ => None,
        }
    }

    pub(crate) fn edit_box_mut(&mut self, index: usize) -> Option<&mut EditBox> {
        match &mut self.widgets.get_mut(index)?.kind {
            WidgetKind::EditBox(edit) => Some(edit),
            _ => None,
        }
    }
}

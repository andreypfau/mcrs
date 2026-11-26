use alloc::string::String;
use core::ops::Range;

use crate::anchor::Anchor;
use crate::edit_box_frame::EditBoxFrame;
use crate::edit_box_highlight::EditBoxHighlight;
use crate::edit_box_look::EditBoxLook;
use crate::edit_box_part::EditBoxPart;
use crate::edit_box_run::EditBoxRun;
use crate::gui_key::GuiKey;
use crate::gui_point::GuiPoint;
use crate::gui_rect::GuiRect;
use crate::key_modifiers::KeyModifiers;
use crate::length_unit::LengthUnit;
use crate::step::Step;

const DEFAULT_MAX_LENGTH: usize = 32;
const BORDER_INSET: i32 = 4;
const GLYPH_HEIGHT: i32 = 8;
const CURSOR_HEIGHT: i32 = 11;
const BLINK_MS: u64 = 300;

fn is_allowed(c: char) -> bool {
    !matches!(c, '\0'..='\u{1f}' | '\u{7f}' | '\u{a7}')
}

#[derive(Clone, Debug)]
pub struct EditBox {
    text: String,
    caret: usize,
    anchor: usize,
    first: usize,
    max_length: usize,
    unit: LengthUnit,
    editing: bool,
    focus_ms: u64,
    look: EditBoxLook,
    width_limit: Option<i32>,
    editable: bool,
    filter: Option<fn(&str) -> bool>,
}

struct View {
    origin: GuiPoint,
    inner: i32,
    visible: Range<usize>,
}

impl Default for EditBox {
    fn default() -> Self {
        Self::new(EditBoxLook::default())
    }
}

fn previous_boundary(text: &str, at: usize) -> usize {
    text.get(..at)
        .and_then(|before| before.char_indices().next_back())
        .map_or(0, |(index, _)| index)
}

fn next_boundary(text: &str, at: usize) -> usize {
    text.get(at..)
        .and_then(|after| after.chars().next())
        .map_or(text.len(), |c| at + c.len_utf8())
}

fn previous_word(text: &str, at: usize) -> usize {
    let before = text.get(..at).unwrap_or_default();
    let trimmed = before.trim_end_matches(' ');
    trimmed.rfind(' ').map_or(0, |index| index + 1)
}

fn next_word(text: &str, at: usize) -> usize {
    let after = text.get(at..).unwrap_or_default();
    let word = after.find(' ').unwrap_or(after.len());
    let rest = after.get(word..).unwrap_or_default();
    at + word + (rest.len() - rest.trim_start_matches(' ').len())
}

fn push_allowed(value: &mut String, text: &str, unit: LengthUnit, mut room: usize) -> usize {
    let before = value.len();
    for piece in text.split(|c| !is_allowed(c)) {
        let kept = piece.get(..unit.head(piece, room)).unwrap_or_default();
        value.push_str(kept);
        room = room.saturating_sub(unit.length(kept));
        if kept.len() < piece.len() {
            break;
        }
    }
    value.len() - before
}

fn head(text: &str, room: i32, width: &impl Fn(&str) -> i32) -> usize {
    let mut end = 0;
    for (index, c) in text.char_indices() {
        let next = index + c.len_utf8();
        if width(text.get(..next).unwrap_or_default()) > room {
            break;
        }
        end = next;
    }
    end
}

fn fitting_start(text: &str, target: usize, inner: i32, width: &impl Fn(&str) -> i32) -> usize {
    let mut start = target;
    while start > 0 {
        let before = previous_boundary(text, start);
        if width(text.get(before..target).unwrap_or_default()) > inner {
            break;
        }
        start = before;
    }
    start
}

impl EditBox {
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub fn caret(&self) -> usize {
        self.caret
    }

    #[must_use]
    pub fn selection(&self) -> Range<usize> {
        self.caret.min(self.anchor)..self.caret.max(self.anchor)
    }

    #[must_use]
    pub fn selected(&self) -> &str {
        self.text.get(self.selection()).unwrap_or_default()
    }

    #[must_use]
    pub const fn is_editing(&self) -> bool {
        self.editing
    }

    #[must_use]
    pub fn new(look: EditBoxLook) -> Self {
        Self {
            text: String::new(),
            caret: 0,
            anchor: 0,
            first: 0,
            max_length: DEFAULT_MAX_LENGTH,
            unit: LengthUnit::CodeUnits,
            editing: false,
            focus_ms: 0,
            look,
            width_limit: None,
            editable: true,
            filter: None,
        }
    }

    #[must_use]
    pub fn with_width_limit(mut self, limit: i32) -> Self {
        self.width_limit = Some(limit);
        self
    }

    #[must_use]
    pub fn with_filter(mut self, accepts: fn(&str) -> bool) -> Self {
        self.filter = Some(accepts);
        self
    }

    #[must_use]
    pub const fn is_editable(&self) -> bool {
        self.editable
    }

    pub fn set_editable(&mut self, editable: bool) {
        self.editable = editable;
    }

    #[must_use]
    pub const fn keeps_focus(&self) -> bool {
        !self.look.can_lose_focus()
    }

    pub fn focus(&mut self, now_ms: u64) {
        self.focus_ms = now_ms;
        self.editing = true;
    }

    pub fn focus_without_editing(&mut self, now_ms: u64) {
        self.focus_ms = now_ms;
        self.editing = false;
    }

    pub fn blur(&mut self) {
        if self.look.can_lose_focus() {
            self.editing = false;
        }
    }

    pub fn release(&mut self) {
        self.editing = false;
    }

    pub fn set_max_length(&mut self, max: usize, unit: LengthUnit) -> bool {
        self.max_length = max;
        self.unit = unit;
        let kept = unit.head(&self.text, max);
        if kept == self.text.len() {
            return false;
        }
        self.text.truncate(kept);
        self.caret = self.caret.min(kept);
        self.anchor = self.anchor.min(kept);
        true
    }

    pub fn set(&mut self, text: &str) {
        let kept = self.unit.head(text, self.max_length);
        self.text.clear();
        self.text.push_str(text.get(..kept).unwrap_or_default());
        self.caret = self.text.len();
        self.anchor = self.caret;
    }

    pub fn set_caret(&mut self, caret: usize, anchor: Anchor) {
        let caret = self.text.floor_char_boundary(caret);
        self.caret = caret;
        if anchor == Anchor::Move {
            self.anchor = caret;
        }
    }

    pub fn insert(&mut self, text: &str) -> bool {
        self.replace_selection(text, &|_| 0).is_some()
    }

    pub fn paste(&mut self, text: &str, width: &impl Fn(&str) -> i32) -> bool {
        self.replace_selection(text, width) == Some(true)
    }

    fn replace_selection(&mut self, text: &str, width: &impl Fn(&str) -> i32) -> Option<bool> {
        if !self.editable {
            return None;
        }
        let Range { start, end } = self.selection();
        let kept_length = self
            .unit
            .length(&self.text)
            .saturating_sub(self.unit.length(self.selected()));
        let free = self.max_length.saturating_sub(kept_length);
        if free == 0 {
            return None;
        }
        if !self.text.is_char_boundary(start) || !self.text.is_char_boundary(end) {
            return None;
        }
        let old_len = self.text.len();
        self.text.extend_from_within(..start);
        let inserted = push_allowed(&mut self.text, text, self.unit, free);
        if inserted == 0 && start == end {
            self.text.truncate(old_len);
            return None;
        }
        self.text.extend_from_within(end..old_len);
        let (old, value) = self.text.split_at_checked(old_len).unwrap_or_default();
        let refused = self.filter.is_some_and(|accepts| !accepts(value))
            || self.width_limit.is_some_and(|limit| {
                let new_width = width(value);
                new_width > limit && new_width > width(old)
            });
        let changed = old != value;
        if refused {
            self.text.truncate(old_len);
            return None;
        }
        self.text.replace_range(..old_len, "");
        self.caret = start + inserted;
        self.anchor = self.caret;
        Some(changed)
    }

    fn remove_to(&mut self, to: usize) -> bool {
        let Range { start, end } = if self.caret != self.anchor {
            self.selection()
        } else {
            to.min(self.caret)..to.max(self.caret)
        };
        if start == end {
            return false;
        }
        self.text.replace_range(start..end, "");
        self.caret = start;
        self.anchor = start;
        true
    }

    fn previous(&self, step: Step) -> usize {
        match step {
            Step::Char => previous_boundary(&self.text, self.caret),
            Step::Word => previous_word(&self.text, self.caret),
        }
    }

    fn next(&self, step: Step) -> usize {
        match step {
            Step::Char => next_boundary(&self.text, self.caret),
            Step::Word => next_word(&self.text, self.caret),
        }
    }

    pub fn backspace(&mut self, step: Step) -> bool {
        self.editable && self.remove_to(self.previous(step))
    }

    pub fn delete(&mut self, step: Step) -> bool {
        self.editable && self.remove_to(self.next(step))
    }

    pub fn left(&mut self, step: Step, anchor: Anchor) {
        self.set_caret(self.previous(step), anchor);
    }

    pub fn right(&mut self, step: Step, anchor: Anchor) {
        self.set_caret(self.next(step), anchor);
    }

    pub fn home(&mut self, anchor: Anchor) {
        self.set_caret(0, anchor);
    }

    pub fn end(&mut self, anchor: Anchor) {
        self.set_caret(self.text.len(), anchor);
    }

    pub fn select_all(&mut self) {
        self.anchor = 0;
        self.caret = self.text.len();
        self.first = 0;
    }

    pub fn cut(&mut self) -> bool {
        self.editable && self.caret != self.anchor && self.remove_to(self.caret)
    }

    pub fn draw<'a, H>(
        &'a self,
        frame: EditBoxFrame<'a, H>,
        width: &impl Fn(&str) -> i32,
        runs: &[EditBoxRun],
        out: &mut impl FnMut(EditBoxPart<'a, H>),
    ) {
        let EditBoxFrame {
            rect,
            now_ms,
            hint,
            suggestion,
            highlight,
        } = frame;
        let View {
            origin, visible, ..
        } = self.view(rect, width);
        let (first, last) = (visible.start, visible.end);
        let shown = self.text.get(visible.clone()).unwrap_or_default();
        let shown_width = width(shown);
        let underscore =
            self.caret == self.text.len() && self.unit.length(&self.text) < self.max_length;
        let cursor_x = if !underscore {
            let before = self.text.get(first..self.caret).unwrap_or_default();
            origin.x.saturating_add(width(before))
        } else if first == last {
            origin.x
        } else {
            origin.x.saturating_add(shown_width).saturating_add(1)
        };
        let in_view = first <= self.caret && self.caret <= last;
        let blink_on = (now_ms.saturating_sub(self.focus_ms) / BLINK_MS).is_multiple_of(2);
        let top = origin.y.saturating_sub(1);
        let color = if self.editable {
            self.look.text_color()
        } else {
            EditBoxLook::UNEDITABLE_COLOR
        };
        if self.look.border() {
            out(EditBoxPart::Frame {
                rect,
                highlighted: self.editing || highlight == EditBoxHighlight::On,
            });
        }
        self.draw_text(origin, visible, runs, color, width, out);
        if let Some(hint) = hint
            && self.text.is_empty()
            && !self.editing
        {
            out(EditBoxPart::Hint { at: origin, hint });
        }
        if let Some(text) = suggestion
            && underscore
        {
            let x = if first == last {
                origin.x.saturating_sub(1)
            } else {
                origin.x.saturating_add(shown_width)
            };
            out(EditBoxPart::Suggestion {
                at: GuiPoint::new(x, origin.y),
                text,
                color: EditBoxLook::SUGGESTION_COLOR,
            });
        }
        if self.anchor != self.caret {
            let right = rect.x.saturating_add(rect.width);
            let anchor = self.anchor.max(first).min(last);
            let before = self.text.get(first..anchor).unwrap_or_default();
            let anchor_x = origin.x.saturating_add(width(before)).saturating_sub(1);
            let cursor_x = if in_view {
                cursor_x
            } else if self.caret > last {
                right
            } else {
                origin.x
            };
            let (anchor_x, cursor_x) = (anchor_x.min(right), cursor_x.min(right));
            let left = anchor_x.min(cursor_x);
            let span = anchor_x.max(cursor_x).saturating_sub(left);
            out(EditBoxPart::Selection {
                rect: GuiRect::new(left, top, span, CURSOR_HEIGHT),
                invert: self.look.invert(),
            });
        }
        if self.editing && blink_on && in_view {
            if underscore {
                out(EditBoxPart::CaretUnderscore {
                    at: GuiPoint::new(cursor_x, origin.y),
                    color,
                });
            } else {
                out(EditBoxPart::CaretBar {
                    rect: GuiRect::new(cursor_x, top, 1, CURSOR_HEIGHT),
                    color,
                });
            }
        }
    }

    fn draw_text<'a, H>(
        &'a self,
        origin: GuiPoint,
        visible: Range<usize>,
        runs: &[EditBoxRun],
        color: u32,
        width: &impl Fn(&str) -> i32,
        out: &mut impl FnMut(EditBoxPart<'a, H>),
    ) {
        if runs.is_empty() {
            out(EditBoxPart::Text {
                at: origin,
                text: self.text.get(visible.clone()).unwrap_or_default(),
                color,
            });
            return;
        }
        for run in runs {
            let from = run.range.start.max(visible.start);
            let to = run.range.end.min(visible.end);
            let Some(text) = self.text.get(from..to).filter(|text| !text.is_empty()) else {
                continue;
            };
            let before = self.text.get(visible.start..from).unwrap_or_default();
            out(EditBoxPart::Text {
                at: GuiPoint::new(origin.x.saturating_add(width(before)), origin.y),
                text,
                color: run.color,
            });
        }
    }

    pub(crate) fn key_press(&mut self, key: GuiKey, modifiers: KeyModifiers) -> Option<bool> {
        if !self.editing {
            self.editing = key.is_activation();
            return self.editing.then_some(false);
        }
        let step = if modifiers.control {
            Step::Word
        } else {
            Step::Char
        };
        let anchor = if modifiers.shift {
            Anchor::Keep
        } else {
            Anchor::Move
        };
        match key {
            GuiKey::Backspace => return Some(self.backspace(step)),
            GuiKey::Delete => return Some(self.delete(step)),
            GuiKey::Left => self.left(step, anchor),
            GuiKey::Right => self.right(step, anchor),
            GuiKey::Home => self.home(anchor),
            GuiKey::End => self.end(anchor),
            _ => return None,
        }
        Some(false)
    }

    pub fn click(&mut self, rect: GuiRect, x: f64, anchor: Anchor, width: &impl Fn(&str) -> i32) {
        let position = self.position_at(rect, x, width);
        self.set_caret(position, anchor);
        self.editing = true;
        self.scroll(rect, width);
    }

    pub fn select_word(&mut self, rect: GuiRect, x: f64, width: &impl Fn(&str) -> i32) {
        let position = self.position_at(rect, x, width);
        self.anchor = previous_word(&self.text, position);
        self.caret = next_word(&self.text, position);
        self.editing = true;
        self.scroll(rect, width);
    }

    pub fn drag(&mut self, rect: GuiRect, x: f64, width: &impl Fn(&str) -> i32) {
        let position = self.position_at(rect, x, width);
        self.set_caret(position, Anchor::Keep);
        self.scroll(rect, width);
    }

    pub fn scroll(&mut self, rect: GuiRect, width: &impl Fn(&str) -> i32) {
        self.first = self.text.floor_char_boundary(self.first);
        let view = self.view(rect, width);
        let (first, last) = (view.visible.start, view.visible.end);
        let target = self.caret;
        if target > last || target < first || (target == first && first > 0) {
            self.first = fitting_start(&self.text, target, view.inner, width);
        }
    }

    fn position_at(&self, rect: GuiRect, x: f64, width: &impl Fn(&str) -> i32) -> usize {
        let view = self.view(rect, width);
        let mouse_x = libm::floor(x) as i32;
        let distance = mouse_x.saturating_sub(view.origin.x).min(view.inner);
        if distance < 0 {
            return view.visible.start;
        }
        let shown = self.text.get(view.visible.clone()).unwrap_or_default();
        view.visible.start + head(shown, distance, width)
    }

    fn view(&self, rect: GuiRect, width: &impl Fn(&str) -> i32) -> View {
        let (x, y, inner) = if self.look.border() {
            let inset = rect.height.saturating_sub(GLYPH_HEIGHT).div_euclid(2);
            (
                rect.x.saturating_add(BORDER_INSET),
                rect.y.saturating_add(inset),
                rect.width.saturating_sub(BORDER_INSET * 2),
            )
        } else {
            (rect.x, rect.y, rect.width)
        };
        let first = self.text.floor_char_boundary(self.first);
        let rest = self.text.get(first..).unwrap_or_default();
        let visible = first..first + head(rest, inner, width);
        View {
            origin: GuiPoint::new(x, y),
            inner,
            visible,
        }
    }
}

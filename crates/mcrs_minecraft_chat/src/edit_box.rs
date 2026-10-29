use mcrs_minecraft_protocol::text::is_allowed_chat_character;

use crate::MAX_INPUT;
use mcrs_text_span::Span;

/// How far a caret move or a removal goes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Step {
    /// One character.
    #[default]
    Char,
    /// To the next word boundary.
    Word,
}

/// What a caret move does with the other end of the selection.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Anchor {
    /// The anchor moves with the caret, so nothing stays selected.
    #[default]
    Move,
    /// The anchor stays, so the selection grows or shrinks.
    Keep,
}

/// A one-line text field (CHAT-BOX-001, CHAT-BOX-002). Positions are byte offsets into the text.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EditBox {
    text: String,
    caret: usize,
    anchor: usize,
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

impl EditBox {
    /// The text of the field.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The byte offset of the caret.
    pub fn caret(&self) -> usize {
        self.caret
    }

    /// The selected range between the caret and the anchor.
    pub fn selection(&self) -> Span {
        Span::new(self.caret.min(self.anchor), self.caret.max(self.anchor))
    }

    /// The selected text.
    pub fn selected(&self) -> &str {
        self.selection().text(&self.text)
    }

    /// Replaces the text and puts the caret at its end.
    pub fn set(&mut self, text: &str) {
        self.text.clear();
        self.text.extend(text.chars().take(MAX_INPUT));
        self.caret = self.text.len();
        self.anchor = self.caret;
    }

    /// Moves the caret to the character boundary at or before `caret`.
    pub fn set_caret(&mut self, caret: usize, anchor: Anchor) {
        let caret = self.text.floor_char_boundary(caret);
        self.caret = caret;
        if anchor == Anchor::Move {
            self.anchor = caret;
        }
    }

    /// Types or pastes `typed` over the selection; rejected characters go and the rest is cut
    /// at the limit. Returns whether the text changed.
    pub fn insert(&mut self, typed: &str) -> bool {
        let Span { start, end } = self.selection();
        let kept = self.text.chars().count() - self.selected().chars().count();
        let room = MAX_INPUT.saturating_sub(kept);
        let added: String = typed
            .chars()
            .filter(|&c| is_allowed_chat_character(c))
            .take(room)
            .collect();
        if added.is_empty() && start == end {
            return false;
        }
        self.text.replace_range(start..end, &added);
        self.caret = start + added.len();
        self.anchor = self.caret;
        true
    }

    fn remove_to(&mut self, to: usize) -> bool {
        let Span { start, end } = if self.caret != self.anchor {
            self.selection()
        } else {
            Span::new(to.min(self.caret), to.max(self.caret))
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

    /// Removes the selection, or the step before the caret; returns whether the text changed.
    pub fn backspace(&mut self, step: Step) -> bool {
        self.remove_to(self.previous(step))
    }

    /// Removes the selection, or the step after the caret; returns whether the text changed.
    pub fn delete(&mut self, step: Step) -> bool {
        self.remove_to(self.next(step))
    }

    /// Moves the caret one step back.
    pub fn left(&mut self, step: Step, anchor: Anchor) {
        self.set_caret(self.previous(step), anchor);
    }

    /// Moves the caret one step forward.
    pub fn right(&mut self, step: Step, anchor: Anchor) {
        self.set_caret(self.next(step), anchor);
    }

    /// Moves the caret to the start of the text.
    pub fn home(&mut self, anchor: Anchor) {
        self.set_caret(0, anchor);
    }

    /// Moves the caret to the end of the text.
    pub fn end(&mut self, anchor: Anchor) {
        self.set_caret(self.text.len(), anchor);
    }

    /// Selects the whole text, the caret at its end.
    pub fn select_all(&mut self) {
        self.anchor = 0;
        self.caret = self.text.len();
    }

    /// Removes the selection; returns whether there was one.
    pub fn cut(&mut self) -> bool {
        self.caret != self.anchor && self.remove_to(self.caret)
    }
}

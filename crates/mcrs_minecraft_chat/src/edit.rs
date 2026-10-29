use mcrs_minecraft_protocol::text::is_allowed_chat_character;

use crate::MAX_INPUT;
use mcrs_text_span::{utf16_len, utf16_prefix};

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
    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn caret(&self) -> usize {
        self.caret
    }

    /// The selected range, start first.
    pub fn selection(&self) -> (usize, usize) {
        (self.caret.min(self.anchor), self.caret.max(self.anchor))
    }

    pub fn selected(&self) -> &str {
        let (start, end) = self.selection();
        self.text.get(start..end).unwrap_or_default()
    }

    /// Replaces the text and puts the caret at its end.
    pub fn set(&mut self, text: &str) {
        self.text.clear();
        self.text.push_str(utf16_prefix(text, MAX_INPUT));
        self.caret = self.text.len();
        self.anchor = self.caret;
    }

    pub fn set_caret(&mut self, caret: usize, select: bool) {
        let caret = self.text.floor_char_boundary(caret);
        self.caret = caret;
        if !select {
            self.anchor = caret;
        }
    }

    /// Types or pastes `typed` over the selection; rejected code units go and the rest is cut
    /// at the limit. Returns whether the text changed.
    pub fn insert(&mut self, typed: &str) -> bool {
        let (start, end) = self.selection();
        let kept = utf16_len(&self.text) - utf16_len(self.text.get(start..end).unwrap_or_default());
        let room = MAX_INPUT.saturating_sub(kept);
        let mut added = String::new();
        let mut units = 0;
        for c in typed.chars().filter(|&c| is_allowed_chat_character(c)) {
            units += c.len_utf16();
            if units > room {
                break;
            }
            added.push(c);
        }
        if added.is_empty() && start == end {
            return false;
        }
        self.text.replace_range(start..end, &added);
        self.caret = start + added.len();
        self.anchor = self.caret;
        true
    }

    fn remove_to(&mut self, to: usize) -> bool {
        let (start, end) = if self.caret != self.anchor {
            self.selection()
        } else {
            (to.min(self.caret), to.max(self.caret))
        };
        if start == end {
            return false;
        }
        self.text.replace_range(start..end, "");
        self.caret = start;
        self.anchor = start;
        true
    }

    fn previous(&self, word: bool) -> usize {
        if word {
            previous_word(&self.text, self.caret)
        } else {
            previous_boundary(&self.text, self.caret)
        }
    }

    fn next(&self, word: bool) -> usize {
        if word {
            next_word(&self.text, self.caret)
        } else {
            next_boundary(&self.text, self.caret)
        }
    }

    pub fn backspace(&mut self, word: bool) -> bool {
        self.remove_to(self.previous(word))
    }

    pub fn delete(&mut self, word: bool) -> bool {
        self.remove_to(self.next(word))
    }

    pub fn left(&mut self, word: bool, select: bool) {
        self.set_caret(self.previous(word), select);
    }

    pub fn right(&mut self, word: bool, select: bool) {
        self.set_caret(self.next(word), select);
    }

    pub fn home(&mut self, select: bool) {
        self.set_caret(0, select);
    }

    pub fn end(&mut self, select: bool) {
        self.set_caret(self.text.len(), select);
    }

    pub fn select_all(&mut self) {
        self.anchor = 0;
        self.caret = self.text.len();
    }

    /// Removes the selection; returns whether there was one.
    pub fn cut(&mut self) -> bool {
        self.caret != self.anchor && self.remove_to(self.caret)
    }
}

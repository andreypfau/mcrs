use mcrs_command::SuggestionList;
use mcrs_minecraft_protocol::text::Text;
use mcrs_text_span::Span;
use std::ops::Range;

/// What a suggestion shows when the player points at it.
#[derive(Clone, Debug, PartialEq)]
pub enum Tooltip {
    /// A translation key.
    Key(&'static str),
    /// A text from the server.
    Text(Text),
}

#[derive(Clone, Debug, PartialEq)]
struct Item {
    start: usize,
    text: Range<usize>,
    tooltip: Option<Tooltip>,
}

/// Suggestions over one input, their texts in one buffer.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Entries {
    texts: String,
    items: Vec<Item>,
    range: Span,
}

impl Entries {
    pub(crate) fn clear(&mut self) {
        self.texts.clear();
        self.items.clear();
        self.range = Span::default();
    }

    pub(crate) fn set_range(&mut self, range: Span) {
        self.range = range;
    }

    pub(crate) fn push(
        &mut self,
        start: usize,
        text: impl Iterator<Item = char>,
        tooltip: Option<Tooltip>,
    ) {
        let from = self.texts.len();
        self.texts.extend(text);
        self.items.push(Item {
            start,
            text: from..self.texts.len(),
            tooltip,
        });
    }

    /// The count of suggestions.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether there is no suggestion.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The range of the input that each suggestion replaces, as byte offsets.
    pub fn range(&self) -> Span {
        self.range
    }

    /// The text of suggestion `index`.
    pub fn text(&self, index: usize) -> Option<&str> {
        self.texts.get(self.items.get(index)?.text.clone())
    }

    /// The tooltip of suggestion `index`.
    pub fn tooltip(&self, index: usize) -> Option<&Tooltip> {
        self.items.get(index)?.tooltip.as_ref()
    }

    /// The texts of the suggestions, in list order.
    pub fn iter(&self) -> impl Iterator<Item = &str> {
        (0..self.len()).filter_map(|index| self.text(index))
    }

    /// The start, the text and the tooltip of each suggestion, in list order.
    pub(crate) fn items(&self) -> impl Iterator<Item = (usize, &str, Option<&Tooltip>)> {
        self.items.iter().map(|item| {
            (
                item.start,
                self.texts.get(item.text.clone()).unwrap_or_default(),
                item.tooltip.as_ref(),
            )
        })
    }

    pub(crate) fn fill<T>(
        &mut self,
        list: &SuggestionList<'_, '_, T>,
        tooltip: impl Fn(&T) -> Tooltip,
    ) {
        self.clear();
        self.range = list.span;
        for (text, tip) in list.iter() {
            self.push(list.span.start, text.chars(), tip.map(&tooltip));
        }
    }

    /// Puts each suggestion that starts with the last word first, keeping the order within
    /// each group (CCMD-SUG-003).
    pub(crate) fn order_by_last_word(&mut self, input: &str, caret: usize) {
        let before = input.get(..caret).unwrap_or_default();
        let word_start = before.rfind(is_separator).map_or(0, |index| index + 1);
        let word = before.get(word_start..).unwrap_or_default().to_lowercase();
        let texts = &self.texts;
        self.items.sort_by_key(|item| {
            let text = texts.get(item.text.clone()).unwrap_or_default();
            let first = text.starts_with(&word)
                || text
                    .strip_prefix("minecraft:")
                    .is_some_and(|rest| rest.starts_with(&word));
            !first
        });
    }
}

pub(crate) fn is_separator(c: char) -> bool {
    matches!(c, '\u{9}'..='\u{d}' | ' ')
}

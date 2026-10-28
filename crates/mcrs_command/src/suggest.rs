use crate::error::Span;
use alloc::vec::Vec;
use core::cmp::Ordering;

/// The text of a suggestion: borrowed parts, written one after the other. A caller gives up
/// to two; the list puts the input before its start in front.
#[derive(Clone, Copy, Debug)]
pub struct SuggestionText<'t>([&'t str; 3]);

impl<'t> SuggestionText<'t> {
    pub const fn new(text: &'t str) -> Self {
        Self([text, "", ""])
    }

    pub const fn concat(first: &'t str, second: &'t str) -> Self {
        Self([first, second, ""])
    }

    pub fn chars(&self) -> impl Iterator<Item = char> + 't {
        self.0.into_iter().flat_map(str::chars)
    }

    pub fn eq_str(&self, text: &str) -> bool {
        self.chars().eq(text.chars())
    }

    pub fn starts_with(&self, prefix: &str) -> bool {
        let mut chars = self.chars();
        prefix.chars().all(|c| chars.next() == Some(c))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Suggestion<'t, T> {
    pub start: usize,
    pub text: SuggestionText<'t>,
    pub tooltip: Option<T>,
}

fn fold(chars: impl Iterator<Item = char>) -> impl Iterator<Item = char> {
    chars.flat_map(char::to_lowercase)
}

pub fn eq_ignore_case(a: impl Iterator<Item = char>, b: impl Iterator<Item = char>) -> bool {
    fold(a).eq(fold(b))
}

pub fn starts_with_ignore_case(text: &str, prefix: &str) -> bool {
    let mut text = fold(text.chars());
    fold(prefix.chars()).all(|c| text.next() == Some(c))
}

#[expect(
    clippy::unnecessary_filter_map,
    reason = "the map changes the lifetime of the element type, which a filter cannot"
)]
/// Empties a buffer of suggestions and gives it another lifetime; the allocation stays,
/// because the element types have one layout.
pub fn recycle<'a, 'b, T>(buffer: Vec<Suggestion<'a, T>>) -> Vec<Suggestion<'b, T>> {
    buffer.into_iter().filter_map(|_| None).collect()
}

/// Collects the suggestions for the input up to the cursor, from one start.
pub struct SuggestionBuilder<'t, 'o, T> {
    input: &'t str,
    start: usize,
    out: &'o mut Vec<Suggestion<'t, T>>,
}

impl<'t, 'o, T> SuggestionBuilder<'t, 'o, T> {
    pub fn new(input: &'t str, start: usize, out: &'o mut Vec<Suggestion<'t, T>>) -> Self {
        Self {
            input,
            start: start.min(input.len()),
            out,
        }
    }

    pub fn input(&self) -> &'t str {
        self.input
    }

    pub fn start(&self) -> usize {
        self.start
    }

    pub fn remaining(&self) -> &'t str {
        self.input.get(self.start..).unwrap_or_default()
    }

    /// A builder at another start whose suggestions join this one.
    pub fn at(&mut self, start: usize) -> SuggestionBuilder<'t, '_, T> {
        SuggestionBuilder::new(self.input, start, self.out)
    }

    pub fn suggest(&mut self, text: SuggestionText<'t>, tooltip: Option<T>) {
        if !text.eq_str(self.remaining()) {
            self.out.push(Suggestion {
                start: self.start,
                text,
                tooltip,
            });
        }
    }

    /// Suggests each text that `matches` the remaining input, called as `matches(text, remaining)`.
    pub fn suggest_matching(
        &mut self,
        texts: impl IntoIterator<Item = &'t str>,
        matches: fn(&str, &str) -> bool,
    ) {
        for text in texts {
            if matches(text, self.remaining()) {
                self.suggest(SuggestionText::new(text), None);
            }
        }
    }
}

/// The suggestions of a request, built into one list over the input.
#[derive(Debug)]
pub struct SuggestionList<'b, 't, T> {
    pub span: Span,
    input: &'t str,
    suggestions: &'b [Suggestion<'t, T>],
}

impl<'b, 't, T> SuggestionList<'b, 't, T> {
    /// Each suggestion expanded to the span of the list: the input before its own start, then
    /// its text.
    pub fn iter(&self) -> impl Iterator<Item = (SuggestionText<'t>, Option<&'b T>)> + '_ {
        self.suggestions.iter().map(|suggestion| {
            (
                expanded(self.input, self.span.start, suggestion),
                suggestion.tooltip.as_ref(),
            )
        })
    }
}

fn expanded<'t, T>(
    input: &'t str,
    start: usize,
    suggestion: &Suggestion<'t, T>,
) -> SuggestionText<'t> {
    let prefix = input.get(start..suggestion.start).unwrap_or_default();
    let [a, b, _] = suggestion.text.0;
    SuggestionText([prefix, a, b])
}

fn compare<'t, T>(
    input: &'t str,
    start: usize,
    a: &Suggestion<'t, T>,
    b: &Suggestion<'t, T>,
) -> Ordering {
    let (a, b) = (expanded(input, start, a), expanded(input, start, b));
    fold(a.chars())
        .cmp(fold(b.chars()))
        .then_with(|| a.chars().cmp(b.chars()))
}

/// Sorts and deduplicates the suggestions in place and returns them as one list.
pub fn build_list<'b, 't, T: PartialEq>(
    input: &'t str,
    suggestions: &'b mut Vec<Suggestion<'t, T>>,
) -> SuggestionList<'b, 't, T> {
    let start = suggestions
        .iter()
        .map(|suggestion| suggestion.start)
        .min()
        .unwrap_or(input.len());
    suggestions.sort_unstable_by(|a, b| compare(input, start, a, b));
    let mut kept = 0;
    for index in 0..suggestions.len() {
        let duplicate = {
            let Some(current) = suggestions.get(index) else {
                break;
            };
            suggestions
                .get(..kept)
                .unwrap_or_default()
                .iter()
                .rev()
                .take_while(|earlier| compare(input, start, earlier, current) == Ordering::Equal)
                .any(|earlier| earlier.tooltip == current.tooltip)
        };
        if !duplicate {
            suggestions.swap(kept, index);
            kept += 1;
        }
    }
    suggestions.truncate(kept);
    SuggestionList {
        span: Span::new(start, input.len()),
        input,
        suggestions,
    }
}

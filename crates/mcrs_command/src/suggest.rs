use crate::span::Span;
use alloc::vec::Vec;
use core::cmp::Ordering;

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

    pub fn starts_with(&self, prefix: &str) -> bool {
        let mut chars = self.chars();
        prefix.chars().all(|c| chars.next() == Some(c))
    }
}

impl PartialEq<str> for SuggestionText<'_> {
    fn eq(&self, other: &str) -> bool {
        self.chars().eq(other.chars())
    }
}

impl PartialEq<&str> for SuggestionText<'_> {
    fn eq(&self, other: &&str) -> bool {
        *self == **other
    }
}

#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct Suggestion<'t, T> {
    pub start: usize,
    pub text: SuggestionText<'t>,
    pub tooltip: Option<T>,
}

impl<'t, T> Suggestion<'t, T> {
    pub const fn new(start: usize, text: SuggestionText<'t>, tooltip: Option<T>) -> Self {
        Self {
            start,
            text,
            tooltip,
        }
    }
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

#[must_use]
pub fn recycle<T, U>(buffer: Vec<T>) -> Vec<U> {
    buffer.into_iter().filter_map(|_| None).collect()
}

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

    pub fn at(&mut self, start: usize) -> SuggestionBuilder<'t, '_, T> {
        SuggestionBuilder::new(self.input, start, self.out)
    }

    pub fn suggest(&mut self, text: SuggestionText<'t>, tooltip: Option<T>) {
        if text != self.remaining() {
            self.out.push(Suggestion {
                start: self.start,
                text,
                tooltip,
            });
        }
    }

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

#[derive(Debug)]
pub struct SuggestionList<'b, 't, T> {
    pub span: Span,
    input: &'t str,
    suggestions: &'b [Suggestion<'t, T>],
}

impl<'b, 't, T> SuggestionList<'b, 't, T> {
    #[must_use]
    pub fn build(input: &'t str, suggestions: &'b mut Vec<Suggestion<'t, T>>) -> Self
    where
        T: PartialEq,
    {
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
                    .take_while(|earlier| {
                        compare(input, start, earlier, current) == Ordering::Equal
                    })
                    .any(|earlier| earlier.tooltip == current.tooltip)
            };
            if !duplicate {
                suggestions.swap(kept, index);
                kept += 1;
            }
        }
        suggestions.truncate(kept);
        Self {
            span: Span::new(start, input.len()),
            input,
            suggestions,
        }
    }

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

use crate::error::ResourceError;
use core::fmt;
use mcrs_command::Span;
use mcrs_command::starts_with_ignore_case;
use mcrs_command::{Reader, SuggestionBuilder, SuggestionText, SyntaxError};
use mcrs_minecraft_resource::parse_parts;

/// An identifier borrowed from the command line; a missing namespace is `minecraft`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Identifier<'a> {
    pub namespace: &'a str,
    pub path: &'a str,
}

/// An identifier and the span of the command line it was read from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpannedIdentifier<'a> {
    pub identifier: Identifier<'a>,
    pub span: Span,
}

impl<'a> Identifier<'a> {
    /// The identifier of `text`, or `None` when it is not one.
    #[must_use]
    pub fn parse(text: &'a str) -> Option<Self> {
        let (namespace, path) = parse_parts(text)?;
        Some(Self { namespace, path })
    }
}

/// An identifier equals a string that spells it as `namespace:path`.
impl PartialEq<str> for Identifier<'_> {
    fn eq(&self, canonical: &str) -> bool {
        canonical
            .split_once(':')
            .is_some_and(|(namespace, path)| namespace == self.namespace && path == self.path)
    }
}

impl PartialEq<&str> for Identifier<'_> {
    fn eq(&self, canonical: &&str) -> bool {
        *self == **canonical
    }
}

impl fmt::Display for Identifier<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.namespace, self.path)
    }
}

pub(crate) const fn is_identifier_char(c: char) -> bool {
    matches!(c, '0'..='9' | 'a'..='z' | '_' | ':' | '/' | '.' | '-')
}

pub fn read_identifier<'a, E: From<ResourceError>>(
    reader: &mut Reader<'a>,
) -> Result<SpannedIdentifier<'a>, SyntaxError<E>> {
    let start = reader.cursor();
    let span = reader.read_while(is_identifier_char);
    match Identifier::parse(span.text(reader.input())) {
        Some(identifier) => Ok(SpannedIdentifier { identifier, span }),
        None => {
            reader.set_cursor(start);
            Err(SyntaxError::argument(
                ResourceError::IdInvalid.into(),
                start,
            ))
        }
    }
}

/// `pattern` starts `text` or follows a `.`, `_` or `/` in it.
fn matches_at_separator(pattern: &str, text: &str, starts_with: fn(&str, &str) -> bool) -> bool {
    starts_with(text, pattern)
        || text.match_indices(['.', '_', '/']).any(|(index, _)| {
            text.get(index + 1..)
                .is_some_and(|rest| starts_with(rest, pattern))
        })
}

/// The identifier keeps its case and the remaining input is lowered.
fn matches_identifier_text(pattern: &str, text: &str) -> bool {
    matches_at_separator(pattern, text, |text, pattern| {
        let mut text = text.chars();
        pattern
            .chars()
            .flat_map(char::to_lowercase)
            .all(|c| text.next() == Some(c))
    })
}

/// The remaining input matches a text candidate.
#[must_use]
pub fn text_matches(text: &str, remaining: &str) -> bool {
    matches_at_separator(remaining, text, starts_with_ignore_case)
}

/// `canonical` is `namespace:path`; the remaining input matches the whole of it, or its namespace
/// or path alone.
pub fn identifier_matches(canonical: &str, remaining: &str) -> bool {
    if remaining.contains(':') {
        return matches_identifier_text(remaining, canonical);
    }
    let (namespace, path) = canonical.split_once(':').unwrap_or(("", canonical));
    matches_identifier_text(remaining, namespace) || matches_identifier_text(remaining, path)
}

/// The remaining input matches `prefix` and then `canonical`.
pub fn prefixed_identifier_matches(remaining: &str, prefix: &str, canonical: &str) -> bool {
    if remaining.is_empty() {
        return true;
    }
    let common = remaining
        .char_indices()
        .zip(prefix.chars())
        .find(|((_, a), b)| a != b)
        .map_or(remaining.len().min(prefix.len()), |((index, _), _)| index);
    if common == 0 && !prefix.is_empty() {
        return false;
    }
    remaining
        .get(common..)
        .is_some_and(|rest| identifier_matches(canonical, rest))
}

/// Suggests each id after each prefix that `matches` accepts against the remaining input.
pub fn suggest_prefixed<'t, 'i: 't, T: Copy>(
    builder: &mut SuggestionBuilder<'t, '_, T>,
    prefixes: &[&'static str],
    ids: impl Iterator<Item = (&'i str, Option<T>)>,
    matches: fn(&str, &str, &str) -> bool,
) {
    let remaining = builder.remaining();
    for (id, tooltip) in ids {
        for &prefix in prefixes {
            if matches(remaining, prefix, id) {
                builder.suggest(SuggestionText::concat(prefix, id), tooltip);
            }
        }
    }
}

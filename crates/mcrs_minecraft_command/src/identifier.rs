use crate::error::ResourceError;
use mcrs_command::Span;
use mcrs_command::starts_with_ignore_case;
use mcrs_command::{Reader, SuggestionBuilder, SuggestionText, SyntaxError};
use mcrs_minecraft_resource::ResourceLocation;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpannedIdentifier<'a> {
    pub identifier: ResourceLocation<'a>,
    pub span: Span,
}

pub(crate) const fn is_identifier_char(c: char) -> bool {
    matches!(c, '0'..='9' | 'a'..='z' | '_' | ':' | '/' | '.' | '-')
}

pub fn read_identifier<'a, E: From<ResourceError>>(
    reader: &mut Reader<'a>,
) -> Result<SpannedIdentifier<'a>, SyntaxError<E>> {
    let start = reader.cursor();
    let span = reader.read_while(is_identifier_char);
    match ResourceLocation::read(span.text(reader.input())) {
        Ok(identifier) => Ok(SpannedIdentifier { identifier, span }),
        Err(_) => {
            reader.set_cursor(start);
            Err(SyntaxError::argument(
                ResourceError::IdInvalid.into(),
                start,
            ))
        }
    }
}

fn matches_at_separator(pattern: &str, text: &str, starts_with: fn(&str, &str) -> bool) -> bool {
    starts_with(text, pattern)
        || text.match_indices(['.', '_', '/']).any(|(index, _)| {
            text.get(index + 1..)
                .is_some_and(|rest| starts_with(rest, pattern))
        })
}

fn matches_identifier_text(pattern: &str, text: &str) -> bool {
    matches_at_separator(pattern, text, |text, pattern| {
        let mut text = text.chars();
        pattern
            .chars()
            .flat_map(char::to_lowercase)
            .all(|c| text.next() == Some(c))
    })
}

#[must_use]
pub fn text_matches(text: &str, remaining: &str) -> bool {
    matches_at_separator(remaining, text, starts_with_ignore_case)
}

pub fn identifier_matches(canonical: &str, remaining: &str) -> bool {
    if remaining.contains(':') {
        return matches_identifier_text(remaining, canonical);
    }
    let (namespace, path) = canonical.split_once(':').unwrap_or(("", canonical));
    matches_identifier_text(remaining, namespace) || matches_identifier_text(remaining, path)
}

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

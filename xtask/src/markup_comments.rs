use crate::c_comments::{self, Syntax};
use std::ops::Range;

fn find(text: &str, from: usize, needle: &str) -> Option<usize> {
    text.get(from..)?.find(needle).map(|offset| from + offset)
}

fn find_ignoring_case(text: &str, from: usize, needle: &str) -> Option<usize> {
    let lower = text.get(from..)?.to_ascii_lowercase();
    lower.find(needle).map(|offset| from + offset)
}

fn opens_tag(text: &str, at: usize, name: &str) -> bool {
    let end = at + 1 + name.len();
    text.get(at..at + 1) == Some("<")
        && text
            .get(at + 1..end)
            .is_some_and(|tag| tag.eq_ignore_ascii_case(name))
        && text
            .as_bytes()
            .get(end)
            .is_some_and(|&b| b == b'>' || b.is_ascii_whitespace())
}

fn embedded(found: &mut Vec<Range<usize>>, text: &str, range: Range<usize>, syntax: Syntax) {
    let Some(inner) = text.get(range.clone()) else {
        return;
    };
    found.extend(
        c_comments::comments(inner, syntax)
            .into_iter()
            .map(|r| r.start + range.start..r.end + range.start),
    );
}

pub fn comments(text: &str, astro: bool) -> Vec<Range<usize>> {
    let mut found = Vec::new();
    let mut at = 0;
    if astro
        && text.starts_with("---")
        && let Some(close) = find(text, 3, "\n---")
    {
        embedded(&mut found, text, 3..close, c_comments::SCRIPT);
        at = close + 4;
    }
    while at < text.len() {
        if text.get(at..at + 4) == Some("<!--") {
            let end = find(text, at + 4, "-->").map_or(text.len(), |end| end + 3);
            found.push(at..end);
            at = end;
            continue;
        }
        let region = [("script", c_comments::SCRIPT), ("style", c_comments::CSS)]
            .into_iter()
            .find(|(name, _)| opens_tag(text, at, name));
        if let Some((name, syntax)) = region {
            let content = find(text, at, ">").map_or(text.len(), |end| end + 1);
            let close =
                find_ignoring_case(text, content, &format!("</{name}")).unwrap_or(text.len());
            embedded(&mut found, text, content..close, syntax);
            at = close.max(at + 1);
            continue;
        }
        if astro && text.get(at..at + 1) == Some("{") {
            let inner_start = at + 1;
            let (inner, length) = c_comments::expression(text.get(inner_start..).unwrap_or(""));
            let close = (inner_start + length).min(text.len());
            let mut rest = text.get(inner_start..close).unwrap_or("").to_owned();
            for range in inner.iter().rev() {
                rest.replace_range(range.clone(), "");
            }
            if !inner.is_empty() && rest.trim().is_empty() {
                found.push(at..(close + 1).min(text.len()));
            } else {
                found.extend(
                    inner
                        .into_iter()
                        .map(|r| r.start + inner_start..r.end + inner_start),
                );
            }
            at = close + 1;
            continue;
        }
        at += text
            .get(at..)
            .and_then(|s| s.chars().next())
            .map_or(1, char::len_utf8);
    }
    found
}

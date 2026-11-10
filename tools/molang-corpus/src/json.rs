use serde_json::{Map, Value};

/// Parses JSON with the extensions that pack files use: a byte order mark, `//` and `/* */`
/// comments, and a trailing comma before `]` or `}`.
pub fn parse(text: &str) -> serde_json::Result<Value> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    serde_json::from_str(&drop_trailing_commas(&drop_comments(text)))
}

fn drop_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut in_string = false;
    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            match c {
                '\\' => out.extend(chars.next()),
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match (c, chars.peek()) {
            ('/', Some('/')) => while chars.next_if(|next| *next != '\n').is_some() {},
            ('/', Some('*')) => {
                chars.next();
                while let Some(next) = chars.next() {
                    if next == '*' && chars.next_if_eq(&'/').is_some() {
                        break;
                    }
                }
            }
            _ => {
                in_string = c == '"';
                out.push(c);
            }
        }
    }
    out
}

fn drop_trailing_commas(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_string = false;
    let mut chars = text.char_indices();
    while let Some((at, c)) = chars.next() {
        if in_string {
            out.push(c);
            match c {
                '\\' => out.extend(chars.next().map(|(_, escaped)| escaped)),
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        let closes = c == ','
            && text
                .get(at + 1..)
                .unwrap_or_default()
                .trim_start()
                .starts_with([']', '}']);
        if !closes {
            in_string = c == '"';
            out.push(c);
        }
    }
    out
}

pub struct Member<'value> {
    pub key: &'value String,
    pub value: &'value Value,
}

/// The members of an object in key order, and none for any other value.
pub fn members(value: &Value) -> impl Iterator<Item = Member<'_>> {
    value
        .as_object()
        .into_iter()
        .flat_map(Map::iter)
        .map(|(key, value)| Member { key, value })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_extensions_leave_string_contents_alone() {
        let text = "\u{feff}{ // line\n \"a\": \"x // y, ]\", /* block */ \"b\": [1, 2,], }";
        let value = parse(text).unwrap();
        assert_eq!(value["a"], "x // y, ]");
        assert_eq!(value["b"].as_array().map(Vec::len), Some(2));
    }
}

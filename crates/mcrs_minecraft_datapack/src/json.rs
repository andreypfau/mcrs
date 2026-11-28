use std::borrow::Cow;

use serde::de::DeserializeOwned;
use serde_json::error::Category;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonError {
    pub malformed: bool,
    pub field: Option<String>,
    pub message: String,
}

impl JsonError {
    fn new(error: &serde_json::Error, field: Option<String>) -> Self {
        Self {
            malformed: !matches!(error.classify(), Category::Data),
            field,
            message: error.to_string(),
        }
    }
}

const MAX_DEPTH: usize = 128;

fn depth_exceeds_limit(text: &str) -> bool {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for byte in text.bytes() {
        if in_string {
            match (escaped, byte) {
                (true, _) => escaped = false,
                (false, b'\\') => escaped = true,
                (false, b'"') => in_string = false,
                (false, _) => {}
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'[' | b'{' => {
                depth = depth.saturating_add(1);
                if depth > MAX_DEPTH {
                    return true;
                }
            }
            b']' | b'}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    false
}

fn text(bytes: &[u8]) -> Cow<'_, str> {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    String::from_utf8_lossy(bytes)
}

pub fn read_json<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, JsonError> {
    let text = text(bytes);
    if text.trim_matches([' ', '\t', '\n', '\r']).is_empty() {
        return Err(JsonError {
            malformed: false,
            field: None,
            message: "the file holds no value".to_owned(),
        });
    }
    if depth_exceeds_limit(&text) {
        return Err(JsonError {
            malformed: true,
            field: None,
            message: format!("the value nests deeper than {MAX_DEPTH} levels"),
        });
    }
    let mut json = serde_json::Deserializer::from_str(&text);
    json.disable_recursion_limit();
    let mut track = serde_path_to_error::Track::new();
    let value = T::deserialize(serde_path_to_error::Deserializer::new(
        &mut json, &mut track,
    ));
    let value = value.map_err(|error| {
        let path = track.path();
        let field = path.iter().next().is_some().then(|| path.to_string());
        JsonError::new(&error, field)
    })?;
    json.end().map_err(|error| JsonError::new(&error, None))?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, serde::Deserialize)]
    struct Nested(#[expect(dead_code, reason = "only the decode is tested")] Vec<Nested>);

    #[test]
    fn a_decoded_value_nests_at_most_128_levels() {
        for (depth, loads) in [(128, true), (129, false)] {
            let text = format!("{}{}", "[".repeat(depth), "]".repeat(depth));
            let read = read_json::<Nested>(text.as_bytes());
            assert_eq!(read.is_ok(), loads, "depth {depth}: {read:?}");
        }
    }
}

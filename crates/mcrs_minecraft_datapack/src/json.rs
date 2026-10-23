use std::borrow::Cow;

use serde::de::DeserializeOwned;
use serde_json::error::Category;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonError {
    /// The file is not JSON at all, as opposed to JSON of the wrong shape.
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

fn text(bytes: &[u8]) -> Cow<'_, str> {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    String::from_utf8_lossy(bytes)
}

/// Reads one data pack file as the game does: a byte order mark is skipped, an empty file is an
/// error of its shape, and anything after the value fails.
pub fn read_json<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, JsonError> {
    let text = text(bytes);
    if text.trim_matches([' ', '\t', '\n', '\r']).is_empty() {
        return Err(JsonError {
            malformed: false,
            field: None,
            message: "the file holds no value".to_owned(),
        });
    }
    let mut json = serde_json::Deserializer::from_str(&text);
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

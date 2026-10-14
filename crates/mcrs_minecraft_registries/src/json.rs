use std::borrow::Cow;
use std::marker::PhantomData;

use serde::de::{DeserializeOwned, DeserializeSeed};
use serde_json::error::Category;

pub(crate) struct ReadError {
    pub(crate) malformed: bool,
    pub(crate) field: Option<String>,
    pub(crate) message: String,
}

impl ReadError {
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

pub(crate) fn read_seed<V, S: for<'de> DeserializeSeed<'de, Value = V>>(
    bytes: &[u8],
    seed: S,
) -> Result<V, ReadError> {
    let text = text(bytes);
    if text.trim_matches([' ', '\t', '\n', '\r']).is_empty() {
        return Err(ReadError {
            malformed: false,
            field: None,
            message: "the file holds no value".to_owned(),
        });
    }
    let mut json = serde_json::Deserializer::from_str(&text);
    let mut track = serde_path_to_error::Track::new();
    let value = seed.deserialize(serde_path_to_error::Deserializer::new(
        &mut json, &mut track,
    ));
    let value = value.map_err(|error| {
        let path = track.path();
        let field = path.iter().next().is_some().then(|| path.to_string());
        ReadError::new(&error, field)
    })?;
    json.end().map_err(|error| ReadError::new(&error, None))?;
    Ok(value)
}

pub(crate) fn read<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, ReadError> {
    read_seed(bytes, PhantomData::<T>)
}

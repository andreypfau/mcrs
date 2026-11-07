use core::fmt;

use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};

/// A JSON value of an entity pack file. Objects keep the order of their keys.
#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Json>),
    Object(JsonObject),
}

/// The members of a JSON object in file order. A key that the file repeats holds its last value.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct JsonObject {
    members: Vec<(String, Json)>,
}

/// A syntax error of a file, at a 1-based line and column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JsonError {
    pub line: usize,
    pub column: usize,
}

impl fmt::Display for JsonError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "JSON syntax error at line {}, column {}",
            self.line, self.column
        )
    }
}

impl core::error::Error for JsonError {}

impl Json {
    /// Parses JSON that can also hold `//` and `/* */` comments and a comma after the last
    /// element of an array or an object.
    pub fn parse_lenient(text: &str) -> Result<Self, JsonError> {
        serde_json::from_str(&strict_text(text)).map_err(|error| JsonError {
            line: error.line(),
            column: error.column(),
        })
    }

    #[must_use]
    pub fn as_object(&self) -> Option<&JsonObject> {
        match self {
            Json::Object(object) => Some(object),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_array(&self) -> Option<&[Json]> {
        match self {
            Json::Array(elements) => Some(elements),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::String(text) => Some(text),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Json::Number(number) => Some(*number),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Json::Bool(value) => Some(*value),
            _ => None,
        }
    }
}

impl JsonObject {
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Json> {
        self.members
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &Json)> {
        self.members
            .iter()
            .map(|(name, value)| (name.as_str(), value))
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.members.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    fn insert(&mut self, key: String, value: Json) {
        match self.members.iter_mut().find(|(name, _)| *name == key) {
            Some((_, held)) => *held = value,
            None => self.members.push((key, value)),
        }
    }
}

/// Replaces comments with spaces and removes a comma before a closing bracket, so that a strict
/// parser reads the text. Newlines stay, so error positions stay correct.
fn strict_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut in_string = false;
    while let Some(next) = chars.next() {
        if in_string {
            out.push(next);
            match next {
                '\\' => out.extend(chars.next()),
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match (next, chars.peek()) {
            ('"', _) => {
                in_string = true;
                out.push(next);
            }
            ('/', Some('/')) => {
                for skipped in chars.by_ref() {
                    if skipped == '\n' {
                        out.push('\n');
                        break;
                    }
                }
            }
            ('/', Some('*')) => {
                chars.next();
                out.push_str("  ");
                let mut previous = ' ';
                for skipped in chars.by_ref() {
                    out.push(if skipped == '\n' { '\n' } else { ' ' });
                    if previous == '*' && skipped == '/' {
                        break;
                    }
                    previous = skipped;
                }
            }
            (']' | '}', _) => {
                drop_trailing_comma(&mut out);
                out.push(next);
            }
            _ => out.push(next),
        }
    }
    out
}

fn drop_trailing_comma(out: &mut String) {
    let trimmed = out.trim_end();
    if trimmed.ends_with(',') {
        let end = trimmed.len();
        out.replace_range(end - 1..end, " ");
    }
}

impl<'de> Deserialize<'de> for Json {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(JsonVisitor)
    }
}

struct JsonVisitor;

impl<'de> Visitor<'de> for JsonVisitor {
    type Value = Json;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON value")
    }

    fn visit_unit<E: de::Error>(self) -> Result<Json, E> {
        Ok(Json::Null)
    }

    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Json, E> {
        Ok(Json::Bool(value))
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Json, E> {
        Ok(Json::Number(value as f64))
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Json, E> {
        Ok(Json::Number(value as f64))
    }

    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Json, E> {
        Ok(Json::Number(value))
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Json, E> {
        Ok(Json::String(value.to_owned()))
    }

    fn visit_string<E: de::Error>(self, value: String) -> Result<Json, E> {
        Ok(Json::String(value))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Json, A::Error> {
        let mut elements = Vec::new();
        while let Some(element) = sequence.next_element()? {
            elements.push(element);
        }
        Ok(Json::Array(elements))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Json, A::Error> {
        let mut object = JsonObject::default();
        while let Some((key, value)) = map.next_entry::<String, Json>()? {
            object.insert(key, value);
        }
        Ok(Json::Object(object))
    }
}

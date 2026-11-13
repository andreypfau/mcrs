use serde::Deserialize;

/// A field that holds one value bare, or a list.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum OneOrMany<T> {
    Many(Vec<T>),
    One(T),
}

impl<T> OneOrMany<T> {
    pub fn as_slice(&self) -> &[T] {
        match self {
            OneOrMany::Many(values) => values,
            OneOrMany::One(value) => core::slice::from_ref(value),
        }
    }
}

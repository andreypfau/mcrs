use std::collections::HashMap;

pub const EN_US: &str = "minecraft/lang/en_us.json";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Language(HashMap<String, String>);

impl Language {
    pub fn from_json(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes).map(Self)
    }

    #[must_use]
    pub fn contains(&self, key: &str) -> bool {
        self.0.contains_key(key)
    }

    #[must_use]
    pub fn find(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(String::as_str)
    }

    #[must_use]
    pub fn get<'a>(&'a self, key: &'a str) -> &'a str {
        self.0.get(key).map_or(key, String::as_str)
    }
}

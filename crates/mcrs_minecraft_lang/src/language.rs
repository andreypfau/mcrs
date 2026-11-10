use std::collections::HashMap;

/// The path of the English language file in a resource pack, under `assets/`.
pub const EN_US: &str = "minecraft/lang/en_us.json";

/// The translations of one language, by translation key.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Language(HashMap<String, String>);

impl Language {
    /// Reads a language file: one JSON object from key to pattern.
    pub fn from_json(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes).map(Self)
    }

    /// Whether the language has a pattern for `key`.
    #[must_use]
    pub fn contains(&self, key: &str) -> bool {
        self.0.contains_key(key)
    }

    /// The pattern of `key`, or the key itself when the language has none, as in the game.
    #[must_use]
    pub fn get<'a>(&'a self, key: &'a str) -> &'a str {
        self.0.get(key).map_or(key, String::as_str)
    }
}

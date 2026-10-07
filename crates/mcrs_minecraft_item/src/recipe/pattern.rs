use std::fmt;

use serde::de::{Error as _, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::Ingredient;

/// The ingredient each pattern symbol stands for, in the order read.
#[derive(Clone, Debug, PartialEq)]
pub struct PatternKey(pub Vec<(char, Ingredient)>);

impl PatternKey {
    pub fn get(&self, symbol: char) -> Option<&Ingredient> {
        self.0
            .iter()
            .find(|(key, _)| *key == symbol)
            .map(|(_, ingredient)| ingredient)
    }
}

impl Serialize for PatternKey {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(self.0.len()))?;
        for (symbol, ingredient) in &self.0 {
            map.serialize_entry(&symbol.to_string(), ingredient)?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for PatternKey {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct KeyVisitor;

        impl<'de> Visitor<'de> for KeyVisitor {
            type Value = PatternKey;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a map of one-character symbols to ingredients")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<PatternKey, A::Error> {
                let mut entries: Vec<(char, Ingredient)> = Vec::new();
                while let Some(symbol) = map.next_key::<String>()? {
                    let mut chars = symbol.chars();
                    let (Some(character), None) = (chars.next(), chars.next()) else {
                        return Err(A::Error::custom(format_args!(
                            "Invalid key entry: '{symbol}' is an invalid symbol (must be 1 character only)."
                        )));
                    };
                    if character == ' ' {
                        return Err(A::Error::custom(
                            "Invalid key entry: ' ' is a reserved symbol.",
                        ));
                    }
                    if entries.iter().any(|(seen, _)| *seen == character) {
                        return Err(A::Error::custom(format_args!(
                            "Duplicate entry for key: '{character}'"
                        )));
                    }
                    entries.push((character, map.next_value()?));
                }
                Ok(PatternKey(entries))
            }
        }

        d.deserialize_map(KeyVisitor)
    }
}

/// Up to three rows of up to three symbols each, every row as wide as the
/// first.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(transparent)]
pub struct Pattern(pub Vec<String>);

impl<'de> Deserialize<'de> for Pattern {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let rows = Vec::<String>::deserialize(d)?;
        if rows.len() > 3 {
            return Err(D::Error::custom(
                "Invalid pattern: too many rows, 3 is maximum",
            ));
        }
        let Some(first) = rows.first() else {
            return Err(D::Error::custom(
                "Invalid pattern: empty pattern not allowed",
            ));
        };
        let width = first.chars().count();
        for row in &rows {
            let row_width = row.chars().count();
            if row_width > 3 {
                return Err(D::Error::custom(
                    "Invalid pattern: too many columns, 3 is maximum",
                ));
            }
            if row_width != width {
                return Err(D::Error::custom(
                    "Invalid pattern: each row must be the same width",
                ));
            }
        }
        Ok(Pattern(rows))
    }
}

/// A shaped recipe's key and pattern agree: every symbol of the pattern is in
/// the key, and every symbol of the key is used.
pub(super) fn check_pattern(key: &PatternKey, pattern: &Pattern) -> Result<(), String> {
    let mut symbols = pattern
        .0
        .iter()
        .flat_map(|row| row.chars())
        .filter(|&symbol| symbol != ' ')
        .peekable();
    if symbols.peek().is_none() {
        return Err("Invalid pattern: empty pattern not allowed".into());
    }
    let mut unused: Vec<char> = key.0.iter().map(|(symbol, _)| *symbol).collect();
    for symbol in symbols {
        if key.get(symbol).is_none() {
            return Err(format!(
                "Pattern references symbol '{symbol}' but it's not defined in the key"
            ));
        }
        unused.retain(|&other| other != symbol);
    }
    if !unused.is_empty() {
        let symbols: Vec<String> = unused.iter().map(char::to_string).collect();
        return Err(format!(
            "Key defines symbols that aren't used in pattern: [{}]",
            symbols.join(", ")
        ));
    }
    Ok(())
}

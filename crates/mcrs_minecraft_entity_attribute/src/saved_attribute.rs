use alloc::string::String;
use alloc::vec::Vec;

use serde::Serialize;
use serde::de::{Error as _, MapAccess};

use crate::attribute_keys::Attribute;
use crate::saved_modifier::SavedModifier;
use crate::tolerant::{Lenient, Tolerant};

/// An attribute compound of the save (`persistence.md`): the attribute, its stored base value
/// and its permanent modifiers in the order of the sequence (ATR-SAV-001).
#[derive(Clone, Debug, Serialize)]
pub struct SavedAttribute {
    /// The attribute; a load leaves out a compound whose `id` names none (ATR-SAV-002).
    pub id: Attribute,
    /// The stored base value; a load reads a missing or bad `base` as 0.
    pub base: f64,
    /// The permanent modifiers; the save leaves the field out when there is none.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub modifiers: Vec<SavedModifier>,
}

impl PartialEq for SavedAttribute {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.base.to_bits() == other.base.to_bits()
            && self.modifiers == other.modifiers
    }
}

const FIELDS: &[&str] = &["id", "base", "modifiers"];

impl Tolerant for SavedAttribute {
    fn from_compound<'de, M: MapAccess<'de>>(mut map: M) -> Result<Option<Self>, M::Error> {
        let mut id = None;
        let mut base = None;
        let mut modifiers = None;
        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "id" => id = map.next_value::<Lenient<Attribute>>()?.0,
                "base" => base = map.next_value::<Lenient<f64>>()?.0,
                "modifiers" => modifiers = map.next_value::<Lenient<Vec<SavedModifier>>>()?.0,
                _ => return Err(M::Error::unknown_field(&key, FIELDS)),
            }
        }
        Ok(id.map(|id| Self {
            id,
            base: base.unwrap_or_default(),
            modifiers: modifiers.unwrap_or_default(),
        }))
    }
}

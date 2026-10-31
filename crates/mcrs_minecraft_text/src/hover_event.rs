use std::fmt;

use serde::de::Visitor;
use serde::{Deserialize, Deserializer, Serialize, de};
use uuid::Uuid;

use crate::tagged_compound::{tagged_compound, variant};
use crate::text::map_text;
use crate::{Text, TextMap, TextSerde, TextTypes};
use mcrs_minecraft_core::codec::IntArray;

/// Action to take when mouse-hovering on the text.
#[derive(Clone, PartialEq, Debug, Serialize)]
#[serde(
    tag = "action",
    rename_all = "snake_case",
    bound(serialize = "I: TextSerde")
)]
pub enum HoverEvent<I: TextTypes> {
    ShowText {
        value: Text<I>,
    },
    ShowItem(Box<I>),
    ShowEntity {
        id: I::EntityKey,
        #[serde(with = "uuid_form")]
        uuid: Uuid,
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<Text<I>>,
    },
}

impl<'de, I: TextSerde> Deserialize<'de> for HoverEvent<I> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(bound = "I: TextSerde")]
        struct ShowText<I: TextTypes> {
            value: Text<I>,
        }

        #[derive(Deserialize)]
        #[serde(bound = "I: TextSerde")]
        struct ShowEntity<I: TextTypes> {
            id: I::EntityKey,
            #[serde(deserialize_with = "uuid_form::deserialize")]
            uuid: Uuid,
            #[serde(default)]
            name: Option<Text<I>>,
        }

        let (action, compound) = tagged_compound(d, "action")?;
        Ok(match action.as_str() {
            "show_text" => HoverEvent::ShowText {
                value: variant::<D, ShowText<I>>(compound)?.value,
            },
            "show_item" => HoverEvent::ShowItem(Box::new(variant::<D, I>(compound)?)),
            "show_entity" => {
                let ShowEntity::<I> { id, uuid, name } = variant::<D, ShowEntity<I>>(compound)?;
                HoverEvent::ShowEntity { id, uuid, name }
            }
            other => {
                return Err(de::Error::unknown_variant(
                    other,
                    &["show_text", "show_item", "show_entity"],
                ));
            }
        })
    }
}

/// Four ints, or on read the hyphenated string of 36 characters.
mod uuid_form {
    use super::*;

    pub(super) fn serialize<S: serde::Serializer>(uuid: &Uuid, s: S) -> Result<S::Ok, S::Error> {
        mcrs_minecraft_profile::uuid_ints(*uuid).serialize(s)
    }

    const HYPHENATED_LENGTH: usize = 36;

    fn from_string(v: &str) -> Result<Uuid, String> {
        Uuid::try_parse(v)
            .ok()
            .filter(|_| v.len() == HYPHENATED_LENGTH && v.matches('-').count() == 4)
            .ok_or_else(|| format!("Invalid UUID string: {v}"))
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Uuid, D::Error> {
        struct UuidVisitor;

        impl<'de> Visitor<'de> for UuidVisitor {
            type Value = Uuid;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a uuid as four ints or a string")
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<Uuid, E> {
                from_string(v).map_err(|e| E::custom(format_args!("Invalid UUID {v}: {e}")))
            }

            fn visit_seq<A: de::SeqAccess<'de>>(self, seq: A) -> Result<Uuid, A::Error> {
                let ints = IntArray::<4>::deserialize(de::value::SeqAccessDeserializer::new(seq))?;
                Ok(mcrs_minecraft_profile::ints_uuid(ints))
            }
        }

        d.deserialize_any(UuidVisitor)
    }
}

impl<A: TextTypes> HoverEvent<A> {
    pub fn map<B: TextTypes, T: TextMap<A, B>>(&self, map: &T) -> Result<HoverEvent<B>, T::Error> {
        Ok(match self {
            HoverEvent::ShowText { value } => HoverEvent::ShowText {
                value: value.map(map).map_err(|e| T::at(e, "value"))?,
            },
            HoverEvent::ShowItem(item) => HoverEvent::ShowItem(Box::new(map.item(item)?)),
            HoverEvent::ShowEntity { id, uuid, name } => HoverEvent::ShowEntity {
                id: map.entity(id).map_err(|e| T::at(e, "id"))?,
                uuid: *uuid,
                name: map_text(name, map, "name")?,
            },
        })
    }
}

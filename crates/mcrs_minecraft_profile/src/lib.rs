use std::fmt;

use mcrs_minecraft_core::ResourceLocation;
use serde::de::{Error as _, MapAccess, SeqAccess, Visitor, value};
use serde::ser::{Error as _, SerializeMap};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

use mcrs_minecraft_core::codec::{BoundedString, IntArray};

/// A property from the game profile.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Property<S = String> {
    pub name: S,
    pub value: S,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<S>,
}

pub const MAX_PROPERTIES: usize = 16;

pub type PlayerName = BoundedString<16>;

/// Printable ASCII, no spaces.
fn player_name(text: &str) -> Result<PlayerName, String> {
    let name = PlayerName::new(text).map_err(|e| e.to_string())?;
    if text.chars().any(|c| c <= ' ' || c >= '\x7f') {
        return Err(format!(
            "Player name contained disallowed characters: '{text}'"
        ));
    }
    Ok(name)
}

/// Four ints, most significant first.
pub fn uuid_ints(id: Uuid) -> IntArray<4> {
    let (msb, lsb) = id.as_u64_pair();
    IntArray([
        (msb >> 32) as i32,
        msb as i32,
        (lsb >> 32) as i32,
        lsb as i32,
    ])
}

pub fn ints_uuid(IntArray([a, b, c, d]): IntArray<4>) -> Uuid {
    let half = |high: i32, low: i32| ((high as u32 as u64) << 32) | low as u32 as u64;
    Uuid::from_u64_pair(half(a, b), half(c, d))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlayerModelType {
    Slim,
    Wide,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct SkinPatch {
    pub texture: Option<ResourceLocation>,
    pub cape: Option<ResourceLocation>,
    pub elytra: Option<ResourceLocation>,
    pub model: Option<PlayerModelType>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GameProfileValue {
    pub id: Uuid,
    pub name: PlayerName,
    pub properties: Vec<Property>,
}

/// `Full` when both the id and the name are known, else `Partial`.
#[derive(Clone, Debug, PartialEq)]
pub enum ProfileIdentity {
    Full(GameProfileValue),
    Partial {
        name: Option<PlayerName>,
        id: Option<Uuid>,
        properties: Vec<Property>,
    },
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct Profile {
    pub profile: ProfileIdentity,
    pub skin: SkinPatch,
}

impl Profile {
    pub fn named(name: &str) -> anyhow::Result<Self> {
        Ok(Profile {
            profile: ProfileIdentity::Partial {
                name: Some(player_name(name).map_err(anyhow::Error::msg)?),
                id: None,
                properties: Vec::new(),
            },
            skin: SkinPatch::default(),
        })
    }
}

impl Serialize for Profile {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(None)?;
        let properties = match &self.profile {
            ProfileIdentity::Full(profile) => {
                map.serialize_entry("id", &uuid_ints(profile.id))?;
                map.serialize_entry("name", &profile.name)?;
                &profile.properties
            }
            ProfileIdentity::Partial {
                name,
                id,
                properties,
            } => {
                if let Some(name) = name {
                    map.serialize_entry("name", name)?;
                }
                if let Some(id) = id {
                    map.serialize_entry("id", &uuid_ints(*id))?;
                }
                properties
            }
        };
        if properties.len() > MAX_PROPERTIES {
            return Err(S::Error::custom(format_args!(
                "List is too long: {}, expected range [0-{MAX_PROPERTIES}]",
                properties.len()
            )));
        }
        if !properties.is_empty() {
            map.serialize_entry("properties", properties)?;
        }
        for (key, texture) in [
            ("texture", &self.skin.texture),
            ("cape", &self.skin.cape),
            ("elytra", &self.skin.elytra),
        ] {
            if let Some(texture) = texture {
                map.serialize_entry(key, texture)?;
            }
        }
        if let Some(model) = &self.skin.model {
            map.serialize_entry("model", model)?;
        }
        map.end()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileRepr {
    #[serde(default)]
    id: Option<IntArray<4>>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default, deserialize_with = "properties")]
    properties: Vec<Property>,
    #[serde(default)]
    texture: Option<ResourceLocation>,
    #[serde(default)]
    cape: Option<ResourceLocation>,
    #[serde(default)]
    elytra: Option<ResourceLocation>,
    #[serde(default)]
    model: Option<PlayerModelType>,
}

impl TryFrom<ProfileRepr> for Profile {
    type Error = String;

    fn try_from(repr: ProfileRepr) -> Result<Self, String> {
        let name = repr.name.as_deref().map(player_name).transpose()?;
        let id = repr.id.map(ints_uuid);
        let profile = match (id, name) {
            (Some(id), Some(name)) => ProfileIdentity::Full(GameProfileValue {
                id,
                name,
                properties: repr.properties,
            }),
            (id, name) => ProfileIdentity::Partial {
                name,
                id,
                properties: repr.properties,
            },
        };
        Ok(Profile {
            profile,
            skin: SkinPatch {
                texture: repr.texture,
                cape: repr.cape,
                elytra: repr.elytra,
                model: repr.model,
            },
        })
    }
}

impl<'de> Deserialize<'de> for Profile {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct ProfileVisitor;

        impl<'de> Visitor<'de> for ProfileVisitor {
            type Value = Profile;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a profile or a player name")
            }

            fn visit_str<E: serde::de::Error>(self, text: &str) -> Result<Profile, E> {
                Profile::named(text).map_err(E::custom)
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Profile, A::Error> {
                ProfileRepr::deserialize(value::MapAccessDeserializer::new(map))?
                    .try_into()
                    .map_err(A::Error::custom)
            }
        }

        d.deserialize_any(ProfileVisitor)
    }
}

/// A list multimap: a value sits under its name, and the names keep the order
/// they first appeared in.
pub fn grouped_by_name(properties: Vec<Property>) -> Vec<Property> {
    let mut grouped: Vec<Property> = Vec::with_capacity(properties.len());
    for property in properties {
        let end = grouped
            .iter()
            .rposition(|p| p.name == property.name)
            .map_or(grouped.len(), |i| i + 1);
        grouped.insert(end, property);
    }
    grouped
}

fn check_property(property: &Property) -> Result<(), String> {
    BoundedString::<64>::new(&*property.name).map_err(|e| e.to_string())?;
    BoundedString::<32767>::new(&*property.value).map_err(|e| e.to_string())?;
    if let Some(signature) = &property.signature {
        BoundedString::<1024>::new(&**signature).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// A list of properties, or on read a map from a name to its values.
fn properties<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Property>, D::Error> {
    struct PropertiesVisitor;

    impl<'de> Visitor<'de> for PropertiesVisitor {
        type Value = Vec<Property>;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("a list of properties or a map of values by name")
        }

        fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<Self::Value, A::Error> {
            let properties = Vec::<Property>::deserialize(value::SeqAccessDeserializer::new(seq))?;
            if properties.len() > MAX_PROPERTIES {
                return Err(A::Error::custom(format_args!(
                    "List is too long: {}, expected range [0-{MAX_PROPERTIES}]",
                    properties.len()
                )));
            }
            for property in &properties {
                check_property(property).map_err(A::Error::custom)?;
            }
            Ok(grouped_by_name(properties))
        }

        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
            let mut properties = Vec::new();
            let mut names = 0;
            while let Some((name, values)) = map.next_entry::<String, Vec<String>>()? {
                names += 1;
                if names > MAX_PROPERTIES {
                    return Err(A::Error::custom(format_args!(
                        "Cannot have more than {MAX_PROPERTIES} properties, but was {names}"
                    )));
                }
                for value in values {
                    let property = Property {
                        name: name.clone(),
                        value,
                        signature: None,
                    };
                    check_property(&property).map_err(A::Error::custom)?;
                    properties.push(property);
                }
            }
            Ok(properties)
        }
    }

    d.deserialize_any(PropertiesVisitor)
}

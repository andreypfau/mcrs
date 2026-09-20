use std::fmt;
use std::io::Write;

use anyhow::ensure;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_nbt::{COMPOUND_ID, INT_ARRAY_ID, LIST_ID, STRING_ID};
use mcrs_minecraft_registry::RegistryLookup;
use serde::de::{Error as _, MapAccess, SeqAccess, Visitor, value};
use serde::ser::{Error as _, SerializeMap};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

use crate::item::component::common::{BoundedString, IntArray};
use crate::item::ctx::{DecodeCtx, EncodeCtx};
use crate::item::harness::Sample;
use crate::profile::Property;
use crate::{Bounded, Decode, Encode};

pub const MAX_PROPERTIES: usize = 16;

pub type PlayerName = BoundedString<16>;

/// `StringUtil.isValidPlayerName`: printable ASCII, no spaces.
fn player_name(text: &str) -> Result<PlayerName, String> {
    let name = PlayerName::new(text).map_err(|e| e.to_string())?;
    if text.chars().any(|c| c <= ' ' || c >= '\x7f') {
        return Err(format!(
            "Player name contained disallowed characters: '{text}'"
        ));
    }
    Ok(name)
}

/// `UUIDUtil.CODEC`: four ints, most significant first.
fn uuid_ints(id: Uuid) -> IntArray<4> {
    let (msb, lsb) = id.as_u64_pair();
    IntArray([
        (msb >> 32) as i32,
        msb as i32,
        (lsb >> 32) as i32,
        lsb as i32,
    ])
}

fn ints_uuid(IntArray([a, b, c, d]): IntArray<4>) -> Uuid {
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

/// `PropertyMap` is a list multimap: a value sits under its name, and the
/// names keep the order they first appeared in.
fn grouped_by_name(properties: Vec<Property>) -> Vec<Property> {
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

/// `ExtraCodecs.PROPERTY_MAP`: a list of properties, or on read a map from
/// a name to its values.
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

#[derive(Encode, Decode)]
struct WireProperty<'a> {
    name: Bounded<&'a str, 64>,
    value: Bounded<&'a str, 32767>,
    signature: Option<Bounded<&'a str, 1024>>,
}

fn encode_properties(properties: &[Property], w: impl Write) -> anyhow::Result<()> {
    ensure!(
        properties.len() <= MAX_PROPERTIES,
        "{} properties exceed the maximum of {MAX_PROPERTIES}",
        properties.len()
    );
    let wire: Vec<WireProperty> = properties
        .iter()
        .map(|p| WireProperty {
            name: Bounded(&p.name),
            value: Bounded(&p.value),
            signature: p.signature.as_deref().map(Bounded),
        })
        .collect();
    wire.encode(w)
}

fn decode_properties(r: &mut &[u8]) -> anyhow::Result<Vec<Property>> {
    let wire = Bounded::<Vec<WireProperty>, MAX_PROPERTIES>::decode(r)?.0;
    Ok(grouped_by_name(
        wire.into_iter()
            .map(|p| Property {
                name: p.name.0.into(),
                value: p.value.0.into(),
                signature: p.signature.map(|s| s.0.into()),
            })
            .collect(),
    ))
}

impl EncodeCtx for Profile {
    fn encode_ctx(&self, _: &dyn RegistryLookup, mut w: impl Write) -> anyhow::Result<()> {
        match &self.profile {
            ProfileIdentity::Full(profile) => {
                true.encode(&mut w)?;
                profile.id.encode(&mut w)?;
                profile.name.encode(&mut w)?;
                encode_properties(&profile.properties, &mut w)?;
            }
            ProfileIdentity::Partial {
                name,
                id,
                properties,
            } => {
                false.encode(&mut w)?;
                name.encode(&mut w)?;
                id.encode(&mut w)?;
                encode_properties(properties, &mut w)?;
            }
        }
        self.skin.texture.encode(&mut w)?;
        self.skin.cape.encode(&mut w)?;
        self.skin.elytra.encode(&mut w)?;
        self.skin
            .model
            .map(|model| model == PlayerModelType::Slim)
            .encode(w)
    }
}

impl DecodeCtx<'_> for Profile {
    fn decode_ctx(_: &dyn RegistryLookup, r: &mut &[u8]) -> anyhow::Result<Self> {
        let profile = if bool::decode(r)? {
            ProfileIdentity::Full(GameProfileValue {
                id: Uuid::decode(r)?,
                name: PlayerName::decode(r)?,
                properties: decode_properties(r)?,
            })
        } else {
            ProfileIdentity::Partial {
                name: Option::decode(r)?,
                id: Option::decode(r)?,
                properties: decode_properties(r)?,
            }
        };
        Ok(Profile {
            profile,
            skin: SkinPatch {
                texture: Option::decode(r)?,
                cape: Option::decode(r)?,
                elytra: Option::decode(r)?,
                model: Option::<bool>::decode(r)?.map(|slim| {
                    if slim {
                        PlayerModelType::Slim
                    } else {
                        PlayerModelType::Wide
                    }
                }),
            },
        })
    }
}

impl Sample for Profile {
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        let mut tags = vec![("", COMPOUND_ID)];
        let properties = match &self.profile {
            ProfileIdentity::Full(profile) => {
                tags.extend([("id", INT_ARRAY_ID), ("name", STRING_ID)]);
                &profile.properties
            }
            ProfileIdentity::Partial {
                name,
                id,
                properties,
            } => {
                if name.is_some() {
                    tags.push(("name", STRING_ID));
                }
                if id.is_some() {
                    tags.push(("id", INT_ARRAY_ID));
                }
                properties
            }
        };
        if !properties.is_empty() {
            tags.push(("properties", LIST_ID));
        }
        if self.skin.model.is_some() {
            tags.push(("model", STRING_ID));
        }
        tags
    }

    fn samples() -> Vec<Self> {
        let property = |name: &str, value: &str, signature: Option<&str>| Property {
            name: name.into(),
            value: value.into(),
            signature: signature.map(Into::into),
        };
        vec![
            Profile::named("Notch").unwrap(),
            Profile {
                profile: ProfileIdentity::Full(GameProfileValue {
                    id: ints_uuid(IntArray([-1, 2, -3, 4])),
                    name: PlayerName::new("Steve").unwrap(),
                    properties: vec![
                        property("textures", "v", Some("s")),
                        property("x", "y", None),
                    ],
                }),
                skin: SkinPatch {
                    texture: Some(ResourceLocation::minecraft("skin")),
                    cape: Some(ResourceLocation::minecraft("cape")),
                    elytra: Some(ResourceLocation::minecraft("elytra")),
                    model: Some(PlayerModelType::Slim),
                },
            },
            Profile {
                profile: ProfileIdentity::Partial {
                    name: None,
                    id: Some(ints_uuid(IntArray([1, 2, 3, 4]))),
                    properties: Vec::new(),
                },
                skin: SkinPatch::default(),
            },
            Profile {
                profile: ProfileIdentity::Partial {
                    name: Some(PlayerName::new("Steve").unwrap()),
                    id: None,
                    properties: vec![
                        property("textures", "v1", None),
                        property("textures", "v2", None),
                    ],
                },
                skin: SkinPatch {
                    model: Some(PlayerModelType::Wide),
                    ..Default::default()
                },
            },
        ]
    }
}

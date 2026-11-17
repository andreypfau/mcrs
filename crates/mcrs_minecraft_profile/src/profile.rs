use mcrs_minecraft_resource::ResourceLocationBuf;
use std::fmt;

use serde::de::{Error as _, MapAccess, Visitor, value};
use serde::ser::{Error as _, SerializeMap};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use mcrs_minecraft_core::codec::IntArray;

use crate::player_name::{check_player_name, player_name};
use crate::property::{check_property, is_grouped_by_name, properties};
use crate::{
    GameProfileValue, MAX_PROPERTIES, PlayerModelType, ProfileError, ProfileIdentity, Property,
    SkinPatch, ints_uuid, uuid_ints,
};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
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

    pub fn check(&self) -> Result<(), ProfileError<'_>> {
        let (name, properties) = match &self.profile {
            ProfileIdentity::Full(profile) => (Some(&profile.name), &profile.properties),
            ProfileIdentity::Partial {
                name, properties, ..
            } => (name.as_ref(), properties),
        };
        if let Some(name) = name {
            check_player_name(&name.0)?;
        }
        for property in properties {
            check_property(property)?;
        }
        if properties.len() > MAX_PROPERTIES {
            return Err(ProfileError::TooManyProperties(properties.len()));
        }
        if !is_grouped_by_name(properties) {
            return Err(ProfileError::UngroupedProperties);
        }
        Ok(())
    }
}

impl Serialize for Profile {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.check().map_err(S::Error::custom)?;
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
struct ProfileRepr {
    #[serde(default)]
    id: Option<IntArray<4>>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default, deserialize_with = "properties")]
    properties: Vec<Property>,
    #[serde(default)]
    texture: Option<ResourceLocationBuf>,
    #[serde(default)]
    cape: Option<ResourceLocationBuf>,
    #[serde(default)]
    elytra: Option<ResourceLocationBuf>,
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

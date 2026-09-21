pub trait Sample: Sized {
    fn samples() -> Vec<Self>;

    /// The NBT tag id each dotted path below this sample's persistent form
    /// must carry, `""` naming the root; a width vanilla would write
    /// differently is a persistence bug the round trip alone cannot see.
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        Vec::new()
    }
}

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::codec::IntArray;
use mcrs_minecraft_nbt::{COMPOUND_ID, INT_ARRAY_ID, LIST_ID, STRING_ID};
use mcrs_minecraft_profile::{
    GameProfileValue, PlayerModelType, PlayerName, Profile, ProfileIdentity, Property, SkinPatch,
    ints_uuid,
};

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

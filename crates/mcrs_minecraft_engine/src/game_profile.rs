use bevy_ecs::component::Component;
use mcrs_minecraft_protocol::profile::Property;
use mcrs_minecraft_protocol::{Bounded, uuid};
use std::borrow::Cow;

/// The profile a client logged in with. The session carries it on the host, and the player
/// carries it in its dimension world.
#[derive(Debug, Clone, Component, serde::Serialize, serde::Deserialize)]
pub struct GameProfile {
    pub id: uuid::Uuid,
    pub username: String,
    pub properties: Vec<Property<String>>,
}

impl<'a> From<&'a GameProfile> for mcrs_minecraft_protocol::profile::GameProfile<'a> {
    fn from(profile: &'a GameProfile) -> Self {
        let props: Vec<Property<&'a str>> = profile
            .properties
            .iter()
            .map(|p| Property {
                name: p.name.as_str(),
                value: p.value.as_str(),
                signature: p.signature.as_deref(),
            })
            .collect();

        Self {
            id: profile.id,
            username: Bounded::from(profile.username.as_str()),
            properties: Cow::Owned(props),
        }
    }
}

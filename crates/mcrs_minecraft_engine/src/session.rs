use bevy_ecs::bundle::Bundle;
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use mcrs_minecraft_protocol::profile::Property;
use mcrs_minecraft_protocol::{Bounded, uuid};
use smallvec::SmallVec;
use std::borrow::Cow;

use crate::bus::InboundPlayerPacket;
use mcrs_minecraft_level::session::{PlayerSession, Session, SessionPlacement};

/// Serverbound packets that arrived while the session had no dimension to take them, handed
/// on in arrival order once it is attached.
#[derive(Component, Default)]
pub struct PendingInbound(pub SmallVec<[InboundPlayerPacket; 4]>);

#[derive(Bundle)]
pub struct SessionBundle {
    session: Session,
    placement: SessionPlacement,
    inbound: PendingInbound,
}

impl SessionBundle {
    pub fn new(session: PlayerSession) -> Self {
        Self::placed(session, SessionPlacement::default())
    }

    pub fn placed(session: PlayerSession, placement: SessionPlacement) -> Self {
        Self {
            session: Session(session),
            placement,
            inbound: PendingInbound::default(),
        }
    }
}

/// On a connection: the host anchor that carries its session.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
#[relationship(relationship_target = SessionConnection)]
pub struct HostAnchorRef(pub Entity);

/// On a player entity of a dimension world: the host anchor that names the player in every
/// dimension, so a dimension system can address its packets without the host's sessions.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostAnchor(pub Entity);

#[derive(Component, Debug)]
#[relationship_target(relationship = HostAnchorRef)]
pub struct SessionConnection(Entity);

impl SessionConnection {
    pub fn entity(&self) -> Entity {
        self.0
    }
}

#[derive(Debug, Clone, Component)]
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

use bevy_ecs::bundle::Bundle;
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use smallvec::SmallVec;

use crate::world::bus::InboundPlayerPacket;
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

#[derive(Component, Debug)]
#[relationship_target(relationship = HostAnchorRef)]
pub struct SessionConnection(Entity);

impl SessionConnection {
    pub fn entity(&self) -> Entity {
        self.0
    }
}

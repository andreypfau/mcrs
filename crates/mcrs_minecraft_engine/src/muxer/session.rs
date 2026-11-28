use bevy_ecs::bundle::Bundle;
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use smallvec::SmallVec;

use crate::bus::InboundPlayerPacket;
use mcrs_minecraft_engine_world::session::{PlayerSession, Session, SessionPlacement};

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

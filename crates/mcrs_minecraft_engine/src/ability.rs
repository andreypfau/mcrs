use bevy_ecs::entity::Entity;
use bevy_ecs::message::Message;
use bevy_ecs::prelude::Component;
use bevy_ecs::schedule::SystemSet;
use derive_more::{Deref, DerefMut};
use mcrs_minecraft_engine_world::session::PlayerSession;

#[derive(
    Component,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Deref,
    DerefMut,
    Default,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct PlayerFlying(pub bool);

#[derive(
    Component,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Deref,
    DerefMut,
    Default,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct PlayerOpLevel(pub u8);

impl PlayerOpLevel {
    pub const MAX: u8 = 4;
    pub const GAMEMASTERS: u8 = 2;

    pub fn clamped(self) -> u8 {
        self.0.min(Self::MAX)
    }

    pub fn entity_status(self) -> i8 {
        24i8 + self.clamped() as i8
    }
}

#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpLevelAnnounced {
    pub player: Entity,
    pub session: PlayerSession,
    pub level: PlayerOpLevel,
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AnnounceOpLevel;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AnswerOpLevel;

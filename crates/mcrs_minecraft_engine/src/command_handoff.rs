use bevy_ecs::entity::Entity;
use bevy_ecs::message::Message;
use bevy_math::DVec3;
use mcrs_engine_muxer::EntityId;
use mcrs_minecraft_command::CommandLevel;
use mcrs_minecraft_core::{Look, LookAnchor};
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::session::PlayerSession;

#[derive(Clone, Debug, PartialEq)]
pub struct SourceState {
    pub level: CommandLevel,
    pub player: Option<PlayerSession>,
    pub mob: Option<SourceMob>,
    pub dimension: Entity,
    pub position: DVec3,
    pub yaw: f32,
    pub pitch: f32,
    pub anchor: LookAnchor,
}

impl SourceState {
    pub fn of_player(
        player: PlayerSession,
        dimension: Entity,
        level: CommandLevel,
        transform: &Transform,
    ) -> Self {
        let Look { yaw, pitch } = transform.rotation.into();
        Self {
            level,
            player: Some(player),
            mob: None,
            dimension,
            position: transform.translation,
            yaw,
            pitch,
            anchor: LookAnchor::Feet,
        }
    }

    pub fn server(dimension: Entity) -> Self {
        Self {
            level: CommandLevel::Owners,
            player: None,
            mob: None,
            dimension,
            position: DVec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            anchor: LookAnchor::Feet,
        }
    }

    #[must_use]
    pub fn with_look(&self, look: Look) -> Self {
        Self {
            yaw: look.yaw,
            pitch: look.pitch,
            ..*self
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceMob {
    pub entity: Entity,
    pub id: EntityId,
}

#[derive(Message, Clone, Debug)]
pub struct CommandHandoff {
    pub to: Entity,
    pub output: PlayerSession,
    pub line: String,
    pub origin: SourceState,
    pub stage: usize,
    pub sources: Vec<SourceState>,
    pub cost: usize,
}

impl CommandHandoff {
    pub fn single(
        to: Entity,
        output: PlayerSession,
        line: String,
        origin: SourceState,
        stage: usize,
    ) -> Self {
        Self {
            to,
            output,
            line,
            sources: vec![origin.clone()],
            origin,
            stage,
            cost: 0,
        }
    }
}

//! A command line whose run goes on in another dimension world, and the source it runs for.

use bevy_ecs::entity::Entity;
use bevy_ecs::message::Message;
use bevy_math::DVec3;
use mcrs_engine_muxer::{EntityId, TileKey};
use mcrs_minecraft_command::CommandLevel;
use mcrs_minecraft_core::{Look, LookAnchor};
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::session::PlayerSession;

/// One source of a command: who runs it, in which dimension, where, facing where, with which
/// permissions. A stage of `execute` maps it to other sources; the output stays the player who
/// sent the line.
#[derive(Clone, Debug, PartialEq)]
pub struct SourceState {
    /// The permission level of the source.
    pub level: CommandLevel,
    /// The session of the player the source holds, which names the player in every
    /// dimension, so the source keeps it when the player changes dimension.
    pub player: Option<PlayerSession>,
    /// The mob the source holds, in the world of the source dimension.
    pub mob: Option<SourceMob>,
    /// The label of the source dimension. The source entity can be in another one.
    pub dimension: Entity,
    /// The position of the source, at its feet.
    pub position: DVec3,
    /// The rotation of the source about the vertical axis, in degrees.
    pub yaw: f32,
    /// The rotation of the source up or down, in degrees.
    pub pitch: f32,
    /// The part of the source that relative and local coordinates start from.
    pub anchor: LookAnchor,
}

impl SourceState {
    /// The source of the player of the session `player`, at its transform in the
    /// dimension labelled `dimension`, at its feet.
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

    /// The source of the server console: no entity, the highest level, at the origin of the
    /// dimension labelled `dimension`.
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

    /// The source looking along `look`.
    #[must_use]
    pub fn with_look(&self, look: Look) -> Self {
        Self {
            yaw: look.yaw,
            pitch: look.pitch,
            ..*self
        }
    }
}

/// A mob row as a command source: its tile when the source was made, and its id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceMob {
    pub tile: TileKey,
    pub id: EntityId,
}

/// The sources of a command line that reached a stage whose handler reads the world of the
/// dimension `to`. That world parses `line` again for `origin` and runs the chain on from
/// `stage`, in its next tick.
#[derive(Message, Clone, Debug)]
pub struct CommandHandoff {
    /// The label of the dimension that runs the rest.
    pub to: Entity,
    /// The player who sent the line, who receives its messages.
    pub output: PlayerSession,
    /// The command line, without its leading slash.
    pub line: String,
    /// The source the line parsed for.
    pub origin: SourceState,
    /// The stage the run goes on from.
    pub stage: usize,
    /// The sources of that stage, in their order.
    pub sources: Vec<SourceState>,
    /// What the stages before it cost, against the sequence limit.
    pub cost: usize,
}

impl CommandHandoff {
    /// The handoff of one source, `origin`, that runs `line` on from `stage` in the dimension
    /// `to`, with nothing spent yet.
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

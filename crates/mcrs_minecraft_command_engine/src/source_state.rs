use crate::command_targets::CommandTargets;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Query, With};
use bevy_math::DVec3;
use mcrs_minecraft_command::CommandLevel;
use mcrs_minecraft_core::{Look, LookAnchor};
use mcrs_minecraft_engine::ability::{PlayerGameMode, PlayerOpLevel};
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_game_mode::GameMode;

/// One source of a command: who runs it, in which dimension, where, facing where, with which
/// permissions. A stage of `execute` maps it to other sources; the output stays the player who
/// sent the line.
#[derive(Clone, Debug, PartialEq)]
pub struct SourceState {
    /// The permission level of the source.
    pub level: CommandLevel,
    /// The host anchor of the player the source holds, which names the player in every
    /// dimension, so the source keeps it when the player changes dimension.
    pub entity: Option<Entity>,
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
    /// The source of the player with the host anchor `anchor`, at its transform in the
    /// dimension labelled `dimension`, at its feet.
    pub fn of_player(
        anchor: Entity,
        dimension: Entity,
        level: CommandLevel,
        transform: &Transform,
    ) -> Self {
        let Look { yaw, pitch } = transform.rotation.into();
        Self {
            level,
            entity: Some(anchor),
            dimension,
            position: transform.translation,
            yaw,
            pitch,
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

/// The command level of a player with `op_level`.
pub fn level_of(op_level: PlayerOpLevel) -> CommandLevel {
    CommandLevel::from(op_level.clamped())
}

pub(crate) type PlayerLevels<'w, 's> =
    Query<'w, 's, (&'static PlayerOpLevel, &'static PlayerGameMode), With<Player>>;

/// The source of the player with the host anchor `player`, from its entry in the targets and its
/// level in this world.
pub(crate) fn player_source(
    targets: &CommandTargets,
    levels: &PlayerLevels,
    player: Entity,
) -> Option<(SourceState, GameMode)> {
    let entry = targets.player(player)?;
    let dimension = targets.label_of(entry.dimension)?;
    let (&op_level, mode) = levels.get(entry.entity).ok()?;
    let state = SourceState::of_player(player, dimension, level_of(op_level), &entry.transform);
    Some((state, mode.0))
}

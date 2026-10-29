use crate::command_targets::CommandTargets;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Query, With};
use bevy_math::DVec3;
use mcrs_minecraft_command::CommandLevel;
use mcrs_minecraft_engine::ability::{PlayerGameMode, PlayerOpLevel};
use mcrs_minecraft_level::entity::physics::Transform;
use mcrs_minecraft_level::entity::player::Player;
use mcrs_minecraft_protocol::{GameMode, LookAnchor};

/// One source of a command: who runs it, in which dimension, where, facing where, with which
/// permissions. A stage of `execute` maps it to other sources; the output stays the player who
/// sent the line.
#[derive(Clone, Debug, PartialEq)]
pub struct SourceState {
    pub level: CommandLevel,
    /// The host anchor of the player the source holds, which names the player in every
    /// dimension, so the source keeps it when the player changes dimension (DIM-CHG-001).
    pub entity: Option<Entity>,
    /// The label of the source dimension. The source entity can be in another one
    /// (ARG-ENT-021).
    pub dimension: Entity,
    pub position: DVec3,
    pub yaw: f32,
    pub pitch: f32,
    pub anchor: LookAnchor,
}

impl SourceState {
    pub fn of_player(
        anchor: Entity,
        dimension: Entity,
        level: CommandLevel,
        transform: &Transform,
    ) -> Self {
        Self {
            level,
            entity: Some(anchor),
            dimension,
            position: transform.translation,
            yaw: transform.rotation.yaw(),
            pitch: transform.rotation.pitch(),
            anchor: LookAnchor::Feet,
        }
    }
}

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

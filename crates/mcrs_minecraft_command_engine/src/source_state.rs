use crate::command_targets::CommandTargets;
use bevy_ecs::prelude::{Query, With};
use mcrs_minecraft_command::CommandLevel;
use mcrs_minecraft_engine::ability::PlayerOpLevel;
pub use mcrs_minecraft_engine::command_handoff::SourceState;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_game_mode::{GameMode, PlayerGameMode};

pub fn level_of(op_level: PlayerOpLevel) -> CommandLevel {
    CommandLevel::from(op_level.clamped())
}

pub(crate) type PlayerLevels<'w, 's> =
    Query<'w, 's, (&'static PlayerOpLevel, &'static PlayerGameMode), With<Player>>;

pub(crate) fn player_source(
    targets: &CommandTargets,
    levels: &PlayerLevels,
    player: PlayerSession,
) -> Option<(SourceState, GameMode)> {
    let entry = targets.player(player)?;
    let dimension = targets.label_of(entry.dimension)?;
    let (&op_level, mode) = levels.get(entry.entity?).ok()?;
    let state = SourceState::of_player(player, dimension, level_of(op_level), &entry.transform);
    Some((state, mode.0))
}

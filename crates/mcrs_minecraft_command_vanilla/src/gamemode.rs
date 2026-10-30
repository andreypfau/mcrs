use crate::select::{FoundPlayers, SelfSource, targets_or_self};
use crate::set::{VanillaCommands, VanillaContext};
use bevy_ecs::prelude::*;
use mcrs_command::{ArgumentHandle, BuildError, NodeId};
use mcrs_minecraft_command::{CommandLevel, PermissionCheck};
use mcrs_minecraft_command_argument::GameModeArgument;
use mcrs_minecraft_command_argument_entity::EntityArgument;
use mcrs_minecraft_command_engine::{
    Call, CommandOutput, CommandRegistrar, CommandResult, EffectQueue, Feedback, Worlds,
};
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_text::{IntoText, Text};

/// The command effect that sets the game mode of the player with the host anchor `player`, in
/// any dimension.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SetGameMode {
    /// The host anchor of the player.
    pub player: Entity,
    /// The game mode the player gets.
    pub mode: GameMode,
}

#[derive(Clone, Copy)]
pub(crate) struct GameModeCommand {
    mode: ArgumentHandle<GameModeArgument>,
    target: ArgumentHandle<EntityArgument>,
}

pub(crate) fn build(
    registrar: &mut CommandRegistrar<'_, VanillaCommands, ()>,
) -> Result<GameModeCommand, BuildError> {
    let gamemode = registrar
        .literal(NodeId::ROOT, "gamemode")
        .requires(PermissionCheck::Require(CommandLevel::Gamemasters))
        .id()?;
    let mode = registrar
        .arg(gamemode, "gamemode", GameModeArgument)
        .runs(())
        .id()?;
    let target = registrar
        .arg(mode, "target", EntityArgument::PLAYERS)
        .runs(())
        .id()?;
    Ok(GameModeCommand { mode, target })
}

fn mode_text(mode: GameMode) -> Text {
    Text::translate(mode.translation_key(), vec![])
}

/// Each target that does not have the mode gets it, in
/// any dimension, and its messages follow.
pub(crate) fn run(
    In(call): In<Call<(), GameModeCommand>>,
    context: VanillaContext,
    mut output: CommandOutput,
    mut modes: ResMut<EffectQueue<SetGameMode>>,
    mut found: Local<FoundPlayers>,
) -> CommandResult {
    let command = call.command;
    let mode = context.arg(&call.invocation, command.mode)?;
    found.with(|targets| {
        targets_or_self(
            &context,
            &call.invocation,
            command.target,
            "argument.entity.notfound.player",
            SelfSource::Player,
            targets,
        )?;
        let mut changed: i32 = 0;
        for &target in targets.found.iter() {
            if target.mode == mode {
                continue;
            }
            changed = changed.saturating_add(1);
            output.effect(
                &mut modes,
                SetGameMode {
                    player: target.anchor,
                    mode,
                },
            );
            if Some(target.anchor) == call.invocation.source.entity {
                output.success(
                    &call.invocation,
                    Text::translate("commands.gamemode.success.self", vec![mode_text(mode)]),
                    Feedback::SourceAndOperators,
                );
            } else {
                output.tell(
                    target.anchor,
                    Text::translate("gameMode.changed", vec![mode_text(mode)]),
                );
                output.success(
                    &call.invocation,
                    Text::translate(
                        "commands.gamemode.success.other",
                        vec![target.name.clone().into_text(), mode_text(mode)],
                    ),
                    Feedback::SourceAndOperators,
                );
            }
        }
        Ok(changed)
    })
}

pub(crate) fn apply(worlds: &mut Worlds, SetGameMode { player, mode }: SetGameMode) {
    if let Err(error) = worlds.set_game_mode(player, mode) {
        tracing::error!(?player, "a game mode change did not apply: {error}");
    }
}

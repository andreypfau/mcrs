use crate::command::vanilla::{FoundPlayers, targets_or_self};
use crate::command::{
    Call, CommandContext, CommandOutput, CommandRegistrar, CommandResult, EffectQueue, Worlds,
};
use bevy_ecs::prelude::*;
use mcrs_command::{ArgumentHandle, BuildError, NodeId};
use mcrs_minecraft_command::{CommandLevel, GameModeArgument, PermissionCheck};
use mcrs_minecraft_command_entity::EntityArgument;
use mcrs_minecraft_protocol::GameMode;
use mcrs_minecraft_protocol::text::{IntoText, Text};

/// The command effect that sets the game mode of the player with the host anchor `player`, in
/// any dimension.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SetGameMode {
    pub player: Entity,
    pub mode: GameMode,
}

#[derive(Clone, Copy)]
pub(super) struct GameModeCommand {
    mode: ArgumentHandle<GameModeArgument>,
    target: ArgumentHandle<EntityArgument>,
}

/// CMD-GM-001.
pub(super) fn build(r: &mut CommandRegistrar<'_, ()>) -> Result<GameModeCommand, BuildError> {
    let gamemode = r
        .literal(NodeId::ROOT, "gamemode")
        .requires(PermissionCheck::Require(CommandLevel::Gamemasters))
        .id()?;
    let mode = r
        .arg(gamemode, "gamemode", GameModeArgument)
        .runs(())
        .id()?;
    let target = r
        .arg(mode, "target", EntityArgument::PLAYERS)
        .runs(())
        .id()?;
    Ok(GameModeCommand { mode, target })
}

fn mode_text(mode: GameMode) -> Text {
    Text::translate(mode.translation_key(), vec![])
}

/// CMD-GM-002 to CMD-GM-005, CMD-GM-009: each target that does not have the mode gets it, in
/// any dimension, and its messages follow.
pub(super) fn run(
    In(call): In<Call<(), GameModeCommand>>,
    context: CommandContext,
    mut output: CommandOutput,
    mut modes: ResMut<EffectQueue<SetGameMode>>,
    mut found: Local<FoundPlayers>,
) -> CommandResult {
    let command = call.command;
    let mode = context.arg(&call, command.mode)?;
    found.with(|targets| {
        targets_or_self(
            &context,
            &call,
            command.target,
            "argument.entity.notfound.player",
            targets,
        )?;
        let mut changed = 0;
        for &target in targets.iter() {
            if target.mode == mode {
                continue;
            }
            changed += 1;
            output.effect(
                &mut modes,
                SetGameMode {
                    player: target.anchor,
                    mode,
                },
            );
            if Some(target.anchor) == call.source.entity {
                output.success(
                    &call,
                    Text::translate("commands.gamemode.success.self", vec![mode_text(mode)]),
                    true,
                );
            } else {
                output.tell(
                    target.anchor,
                    Text::translate("gameMode.changed", vec![mode_text(mode)]),
                );
                output.success(
                    &call,
                    Text::translate(
                        "commands.gamemode.success.other",
                        vec![target.name.clone().into_text(), mode_text(mode)],
                    ),
                    true,
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

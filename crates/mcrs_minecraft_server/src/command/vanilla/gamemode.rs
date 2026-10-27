use crate::command::seam::Worlds;
use crate::command::vanilla::targets_or_self;
use crate::command::{Call, CommandContext, CommandOutput, CommandRegistrar, CommandResult};
use crate::world::entity::player::ability::PlayerGameMode;
use bevy_ecs::prelude::*;
use mcrs_command::{Arg, BuildError, NodeId};
use mcrs_minecraft_command::{CommandLevel, EntityArgument, GameModeArgument, PermissionCheck};
use mcrs_minecraft_protocol::GameMode;
use mcrs_minecraft_protocol::text::{IntoText, Text};

#[derive(Clone, Copy)]
pub(super) struct GameModeCommand {
    mode: Arg<GameModeArgument>,
    target: Arg<EntityArgument>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Branch {
    Source,
    Targets,
}

/// CMD-GM-001.
pub(super) fn build(r: &mut CommandRegistrar<'_, Branch>) -> Result<GameModeCommand, BuildError> {
    let gamemode = r
        .literal(NodeId::ROOT, "gamemode")
        .requires(PermissionCheck::Require(CommandLevel::Gamemasters))
        .id()?;
    let mode = r
        .arg(gamemode, "gamemode", GameModeArgument)
        .runs(Branch::Source)
        .id()?;
    let target = r
        .arg(mode, "target", EntityArgument::PLAYERS)
        .runs(Branch::Targets)
        .id()?;
    Ok(GameModeCommand { mode, target })
}

fn mode_text(mode: GameMode) -> Text {
    Text::translate(format!("gameMode.{}", mode.name()), vec![])
}

/// CMD-GM-002 to CMD-GM-005, CMD-GM-009: each target that does not have the mode gets it, in
/// any dimension, and its messages follow.
pub(super) fn run(
    In(call): In<Call<Branch, GameModeCommand>>,
    context: CommandContext,
    mut output: CommandOutput,
) -> CommandResult {
    let command = call.command;
    let mode = context.arg(&call, command.mode)?;
    let selector = match call.branch {
        Branch::Source => None,
        Branch::Targets => Some(command.target),
    };
    let targets = targets_or_self(&context, &call, selector, "argument.entity.notfound.player")?;
    let mut changed = 0;
    for target in targets {
        if target.mode == mode {
            continue;
        }
        changed += 1;
        output.set_game_mode(target.anchor, mode);
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
}

pub(crate) fn apply(worlds: &mut Worlds, player: Entity, mode: GameMode) {
    let current = worlds
        .locate(player)
        .and_then(|(index, entity)| worlds.world_mut(index)?.get_mut::<PlayerGameMode>(entity));
    match current {
        Some(mut current) => {
            current.set_if_neq(PlayerGameMode(mode));
        }
        None => tracing::error!(?player, "a game mode change names no online player"),
    }
}

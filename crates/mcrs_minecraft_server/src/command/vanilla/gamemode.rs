use crate::command::vanilla::{candidates, targets_or_self};
use crate::command::{Call, CommandContext, CommandFeedback, CommandRegistrar, CommandResult};
use crate::world::entity::player::ability::PlayerGameMode;
use bevy_ecs::prelude::*;
use mcrs_command::{Arg, BuildError, NodeId};
use mcrs_minecraft_command::{CommandLevel, EntityArgument, GameModeArgument, PermissionCheck};
use mcrs_minecraft_engine::session::GameProfile;
use mcrs_minecraft_level::entity::physics::Transform;
use mcrs_minecraft_level::entity::player::Player;
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

pub(super) fn run(
    In(call): In<Call<Branch, GameModeCommand>>,
    context: CommandContext,
    mut players: Query<(Entity, &GameProfile, &Transform, &mut PlayerGameMode), With<Player>>,
    mut feedback: CommandFeedback,
) -> CommandResult {
    let command = call.command;
    let mode = context.arg(&call, command.mode)?;
    let all = candidates(players.iter());
    let selector = match call.branch {
        Branch::Source => None,
        Branch::Targets => Some(command.target),
    };
    let targets = targets_or_self(
        &context,
        &call,
        selector,
        &all,
        "argument.entity.notfound.player",
    )?;
    let mut changed = 0;
    for target in targets {
        let Ok((_, profile, _, mut current)) = players.get_mut(target) else {
            continue;
        };
        if !current.set_if_neq(PlayerGameMode(mode)) {
            continue;
        }
        changed += 1;
        if Some(target) == call.source.entity {
            feedback.success(
                &call,
                Text::translate("commands.gamemode.success.self", vec![mode_text(mode)]),
                true,
            );
        } else {
            feedback.tell(
                target,
                Text::translate("gameMode.changed", vec![mode_text(mode)]),
            );
            feedback.success(
                &call,
                Text::translate(
                    "commands.gamemode.success.other",
                    vec![profile.username.clone().into_text(), mode_text(mode)],
                ),
                true,
            );
        }
    }
    Ok(changed)
}

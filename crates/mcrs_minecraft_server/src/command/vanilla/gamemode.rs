use crate::command::vanilla::{candidates, name, select};
use crate::command::{
    AppCommands, CommandContext, CommandFailure, CommandFeedback, CommandRegistrar, CommandResult,
    Invocation,
};
use crate::world::entity::player::ability::PlayerGameMode;
use bevy_app::App;
use bevy_ecs::prelude::*;
use mcrs_command::{Arg, BuildError};
use mcrs_minecraft_command::{CommandLevel, EntityArgument, GameModeArgument, PermissionCheck};
use mcrs_minecraft_engine::session::GameProfile;
use mcrs_minecraft_level::entity::physics::Transform;
use mcrs_minecraft_level::entity::player::Player;
use mcrs_minecraft_protocol::GameMode;
use mcrs_minecraft_protocol::text::Text;

#[derive(Resource, Clone, Copy)]
struct GameModeCommand {
    mode: Arg<GameModeArgument>,
    target: Arg<EntityArgument>,
}

/// CMD-GM-001.
fn build(r: &mut CommandRegistrar<'_>) -> Result<GameModeCommand, BuildError> {
    let gamemode = r.literal(
        CommandRegistrar::ROOT,
        "gamemode",
        PermissionCheck::Require(CommandLevel::Gamemasters),
    )?;
    let mode = r.argument(
        gamemode,
        "gamemode",
        GameModeArgument,
        PermissionCheck::AlwaysPass,
    )?;
    r.executes(mode.node())?;
    let target = r.argument(
        mode.node(),
        "target",
        EntityArgument::PLAYERS,
        PermissionCheck::AlwaysPass,
    )?;
    r.executes(target.node())?;
    Ok(GameModeCommand { mode, target })
}

pub(super) fn register(app: &mut App) {
    app.add_command(build, run);
}

fn mode_text(mode: GameMode) -> Text {
    Text::translate(format!("gameMode.{}", mode.name()), vec![])
}

fn run(
    In(invocation): In<Invocation>,
    command: Res<GameModeCommand>,
    context: CommandContext,
    mut players: Query<(Entity, &GameProfile, &Transform, &mut PlayerGameMode), With<Player>>,
    mut feedback: CommandFeedback,
) -> CommandResult {
    let lookup = context.lookup();
    let source = invocation.source(&lookup);
    let mode = invocation
        .get(&context.tree, command.mode, &source)
        .ok_or_else(|| CommandFailure::translate("command.failed", vec![]))??;
    let all = candidates(players.iter());
    let targets: Vec<_> = if invocation.has(command.target) {
        let found = select(&context.tree, &invocation, command.target, &source, &all)?;
        if found.is_empty() {
            return Err(CommandFailure::translate(
                "argument.entity.notfound.player",
                vec![],
            ));
        }
        found.into_iter().cloned().collect()
    } else {
        all.iter()
            .filter(|candidate| candidate.entity == invocation.player)
            .cloned()
            .collect()
    };
    let mut changed = 0;
    for target in &targets {
        let Ok((.., mut current)) = players.get_mut(target.entity) else {
            continue;
        };
        if current.0 == mode {
            continue;
        }
        current.0 = mode;
        changed += 1;
        if target.entity == invocation.player {
            feedback.success(
                &invocation,
                Text::translate("commands.gamemode.success.self", vec![mode_text(mode)]),
                true,
            );
        } else {
            feedback.tell(
                target.entity,
                Text::translate("gameMode.changed", vec![mode_text(mode)]),
            );
            feedback.success(
                &invocation,
                Text::translate(
                    "commands.gamemode.success.other",
                    vec![name(target), mode_text(mode)],
                ),
                true,
            );
        }
    }
    Ok(changed)
}

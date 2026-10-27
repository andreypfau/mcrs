use crate::command::{
    AppCommands, Call, CommandContext, CommandFailure, CommandOutput, CommandRegistrar,
    CommandResult, Teleport,
};
use bevy_app::{App, Plugin};
use bevy_ecs::prelude::*;
use bevy_math::DVec3;
use mcrs_command::{Arg, BuildError, NodeId};
use mcrs_minecraft_command::{CommandLevel, DimensionArgument, PermissionCheck};
use mcrs_minecraft_protocol::text::IntoText;

/// Commands of this server that vanilla does not have: `dim <dimension>` moves the source player
/// to `0 128 0` in another dimension, keeping its rotation.
pub struct DebugCommandsPlugin;

impl Plugin for DebugCommandsPlugin {
    fn build(&self, app: &mut App) {
        if let Err(error) = app.add_command(build, run) {
            tracing::error!("dim: {error}");
        }
    }
}

#[derive(Clone, Copy)]
struct DimCommand {
    dimension: Arg<DimensionArgument>,
}

fn build(r: &mut CommandRegistrar<'_, ()>) -> Result<DimCommand, BuildError> {
    let dim = r
        .literal(NodeId::ROOT, "dim")
        .requires(PermissionCheck::Require(CommandLevel::Gamemasters))
        .id()?;
    let dimension = r.arg(dim, "dimension", DimensionArgument).runs(()).id()?;
    Ok(DimCommand { dimension })
}

const ARRIVAL: DVec3 = DVec3::new(0.0, 128.0, 0.0);

fn run(
    In(call): In<Call<(), DimCommand>>,
    context: CommandContext,
    mut output: CommandOutput,
) -> CommandResult {
    let command = call.command;
    let id = context.arg(&call, command.dimension)?;
    let targets = context.targets();
    let dimension = targets
        .dimension_named(id)
        .and_then(|index| targets.dimensions().get(index))
        .ok_or_else(|| {
            CommandFailure::translate(
                "argument.dimension.invalid",
                vec![id.to_string().into_text()],
            )
        })?;
    let player = call
        .source
        .entity
        .filter(|&anchor| targets.player(anchor).is_some())
        .ok_or_else(|| CommandFailure::translate("permissions.requires.player", vec![]))?;
    output.teleport(
        player,
        Teleport {
            dimension: dimension.label,
            position: ARRIVAL,
            rotation: None,
            relative_position: [false; 3],
            relative_rotation: [true; 2],
            facing: None,
            anchor: call.source.anchor,
        },
    );
    Ok(1)
}

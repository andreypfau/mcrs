use crate::select::{FoundPlayers, select};
use crate::set::{VanillaCommands, VanillaContext};
use bevy_ecs::prelude::*;
use mcrs_command::{ArgumentHandle, BuildError, NodeId, StringArgument};
use mcrs_minecraft_command::{CommandLevel, PermissionCheck};
use mcrs_minecraft_command_argument_entity::EntityArgument;
use mcrs_minecraft_command_engine::{
    Call, CommandFailure, CommandOutput, CommandRegistrar, CommandResult, Feedback,
};
use mcrs_minecraft_snbt::{java_double, java_float};
use mcrs_minecraft_text::{IntoText, Text};

#[derive(Clone, Copy)]
pub(crate) struct DataCommand {
    target: ArgumentHandle<EntityArgument>,
    path: ArgumentHandle<StringArgument>,
}

pub(crate) fn build(
    registrar: &mut CommandRegistrar<'_, VanillaCommands, ()>,
) -> Result<DataCommand, BuildError> {
    let data = registrar
        .literal(NodeId::ROOT, "data")
        .requires(PermissionCheck::Require(CommandLevel::Gamemasters))
        .id()?;
    let get = registrar.literal(data, "get").id()?;
    let entity = registrar.literal(get, "entity").id()?;
    let target = registrar
        .arg(entity, "target", EntityArgument::ONE_ENTITY)
        .id()?;
    let path = registrar
        .arg(target, "path", StringArgument::Word)
        .runs(())
        .id()?;
    Ok(DataCommand { target, path })
}

/// `data get entity <target> <path>` for a mob and the paths `Pos`, `Rotation` and `Health`: the
/// value as SNBT in `commands.data.entity.query`. The result is the value as an integer for a
/// number, and the length for a list.
pub(crate) fn run(
    In(call): In<Call<(), DataCommand>>,
    context: VanillaContext,
    mut output: CommandOutput,
    mut found: Local<FoundPlayers>,
) -> CommandResult {
    let command = call.command;
    let path: String = context
        .arg(&call.invocation, command.path)?
        .chars()
        .collect();
    found.with(|targets| {
        select(&context, &call.invocation, command.target, targets)?;
        let Some(mob) = targets.mobs.first() else {
            return Err(CommandFailure::failed());
        };
        let view = context
            .lookup()
            .mob(mob.source())
            .ok_or_else(CommandFailure::failed)?;
        let living = view.state.living.unwrap_or_default();
        let position = view.state.body.position;
        let (value, result) = match path.as_str() {
            "Pos" => (
                format!(
                    "[{}d, {}d, {}d]",
                    java_double(position.x),
                    java_double(position.y),
                    java_double(position.z)
                ),
                3,
            ),
            "Rotation" => (
                format!(
                    "[{}f, {}f]",
                    java_float(living.yaw),
                    java_float(living.pitch)
                ),
                2,
            ),
            "Health" => (
                format!("{}f", java_float(living.health)),
                living.health as i32,
            ),
            _ => {
                return Err(CommandFailure::translate(
                    "arguments.nbtpath.nothing_found",
                    vec![path.clone().into()],
                ));
            }
        };
        output.success(
            &call.invocation,
            Text::translate(
                "commands.data.entity.query",
                vec![view.display_name(), value.into_text()],
            ),
            Feedback::Source,
        );
        Ok(result)
    })
}

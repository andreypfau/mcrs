use bevy_ecs::prelude::*;
use mcrs_command::{ArgumentHandle, BuildError, NodeId};
use mcrs_minecraft_command::{CommandLevel, PermissionCheck};
use mcrs_minecraft_command_argument::TimeArgument;
use mcrs_minecraft_command_engine::{
    Call, CommandOutput, CommandRegistrar, CommandResult, EffectQueue, Feedback,
};
use mcrs_minecraft_command_vanilla::{VanillaCommands, VanillaContext};
use mcrs_minecraft_engine::server_edit::ServerEdit;
use mcrs_minecraft_text::Text;

use crate::server_change::ServerChange;

/// The clear time that `weather clear` without a duration draws from, in ticks.
const CLEAR_TICKS: core::ops::RangeInclusive<u32> = 12_000..=180_000;

#[derive(Clone, Copy)]
pub(crate) struct WeatherCommand {
    duration: ArgumentHandle<TimeArgument>,
}

// `weather rain` and `weather thunder` are not registered: the server has no rain cycle
// and no rain level yet. The rain and thunder of CMD-W-003 lift it.
pub(crate) fn build(
    registrar: &mut CommandRegistrar<'_, VanillaCommands, ()>,
) -> Result<WeatherCommand, BuildError> {
    let weather = registrar
        .literal(NodeId::ROOT, "weather")
        .requires(PermissionCheck::Require(CommandLevel::Gamemasters))
        .id()?;
    let clear = registrar
        .literal(weather, "clear")
        .runs(())
        .in_any_world()
        .id()?;
    let duration = registrar
        .arg(clear, "duration", TimeArgument { min: 1 })
        .runs(())
        .in_any_world()
        .id()?;
    Ok(WeatherCommand { duration })
}

pub(crate) fn run(
    In(call): In<Call<(), WeatherCommand>>,
    context: VanillaContext,
    mut output: CommandOutput,
    mut changes: ResMut<EffectQueue<ServerChange>>,
) -> CommandResult {
    let duration = context.opt(&call.invocation, call.command.duration)?;
    let ticks = match duration {
        Some(ticks) => u32::try_from(ticks).unwrap_or_default(),
        None => rand::random_range(CLEAR_TICKS),
    };
    output.effect(
        &mut changes,
        ServerChange(ServerEdit::ClearWeather { ticks }),
    );
    output.success(
        &call.invocation,
        Text::translate("commands.weather.set.clear", vec![]),
        Feedback::SourceAndOperators,
    );
    Ok(duration.unwrap_or(-1))
}

use bevy_ecs::prelude::*;
use mcrs_minecraft_command_engine::{CommandContext, CommandSet, Invocation};
use mcrs_minecraft_engine::command_handoff::CommandHandoff;
use mcrs_minecraft_engine::dim_mail::DimMail;
use mcrs_minecraft_engine_world::world::sub_app::DimensionLabel;

/// Runs the stage of `invocation` again in the next tick of this world, for the same source.
///
/// A forced column loads in the ticks after `forceload add`, where the game loads it before the
/// command ends; a command that needs it waits for it this way, so a script that forces columns
/// and then fills them works.
pub(crate) fn run_next_tick<S: CommandSet>(
    commands: &mut Commands,
    context: &CommandContext<S>,
    label: &DimensionLabel,
    invocation: &Invocation,
) {
    let handoff = CommandHandoff {
        to: label.0,
        output: invocation.output,
        line: context.line().to_owned(),
        origin: invocation.source.clone(),
        stage: invocation.stage(),
        sources: vec![invocation.source.clone()],
        cost: 0,
    };
    commands.queue(move |world: &mut World| {
        world.get_resource_or_init::<DimMail>().hand_off(handoff);
    });
}

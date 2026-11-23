use bevy_ecs::prelude::*;
use mcrs_minecraft_command_engine::{CommandContext, CommandSet, Invocation};
use mcrs_minecraft_engine::command_handoff::CommandHandoff;
use mcrs_minecraft_engine::dim_mail::DimMail;
use mcrs_minecraft_engine_world::world::sub_app::DimensionLabel;

pub(crate) fn run_next_tick<S: CommandSet>(
    mut commands: Commands<'_, '_>,
    context: &CommandContext<S>,
    label: &DimensionLabel,
    invocation: &Invocation,
) {
    let handoff = CommandHandoff::single(
        label.0,
        invocation.output,
        context.line().to_owned(),
        invocation.source.clone(),
        invocation.stage(),
    );
    commands.queue(move |world: &mut World| {
        world.get_resource_or_init::<DimMail>().hand_off(handoff);
    });
}

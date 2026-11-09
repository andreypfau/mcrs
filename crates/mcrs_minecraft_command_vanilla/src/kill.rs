use crate::mob_hit::HitMob;
use crate::select::{FoundPlayers, SelfSource, targets_or_self};
use crate::set::{VanillaCommands, VanillaContext};
use bevy_ecs::prelude::*;
use mcrs_command::{ArgumentHandle, BuildError, NodeId};
use mcrs_minecraft_command::{CommandLevel, PermissionCheck};
use mcrs_minecraft_command_argument_entity::EntityArgument;
use mcrs_minecraft_command_engine::{
    Call, CommandOutput, CommandRegistrar, CommandResult, EffectQueue, Feedback,
};
use mcrs_minecraft_damage::Hit;
use mcrs_minecraft_resource::rl;
use mcrs_minecraft_text::Text;

use crate::damage::damage_type_id;

#[derive(Clone, Copy)]
pub(crate) struct KillCommand {
    targets: ArgumentHandle<EntityArgument>,
}

pub(crate) fn build(
    registrar: &mut CommandRegistrar<'_, VanillaCommands, ()>,
) -> Result<KillCommand, BuildError> {
    let kill = registrar
        .literal(NodeId::ROOT, "kill")
        .requires(PermissionCheck::Require(CommandLevel::Gamemasters))
        .runs(())
        .id()?;
    let targets = registrar
        .arg(kill, "targets", EntityArgument::ENTITIES)
        .runs(())
        .id()?;
    Ok(KillCommand { targets })
}

/// CMD-KILL-002 to CMD-KILL-006: each mob that the selector finds takes a hit of
/// `generic_kill` with the largest f32 amount, in the order of the selector. The kill of a player
/// has no specification yet, so a player victim counts and takes no hit.
pub(crate) fn run(
    In(call): In<Call<(), KillCommand>>,
    context: VanillaContext,
    mut output: CommandOutput,
    mut hits: ResMut<EffectQueue<HitMob>>,
    mut found: Local<FoundPlayers>,
) -> CommandResult {
    let damage_type = damage_type_id(&context, rl!("minecraft:generic_kill"));
    found.with(|targets| {
        targets_or_self(
            &context,
            &call.invocation,
            call.command.targets,
            "argument.entity.notfound.entity",
            SelfSource::Entity,
            targets,
        )?;
        for mob in &targets.mobs {
            output.effect(
                &mut hits,
                HitMob {
                    at: mob.at,
                    hit: Hit::new(f32::MAX, damage_type),
                },
            );
        }
        let count = i32::try_from(targets.len()).unwrap_or(i32::MAX);
        let message = match (targets.found.first(), targets.mobs.first(), count) {
            (Some(player), _, 1) => Text::translate(
                "commands.kill.success.single",
                vec![player.name.clone().into()],
            ),
            (None, Some(mob), 1) => {
                let name = context
                    .lookup()
                    .mob(mob.source())
                    .map_or_else(|| Text::text(""), |view| view.display_name());
                Text::translate("commands.kill.success.single", vec![name])
            }
            _ => Text::translate_args("commands.kill.success.multiple", vec![count.into()]),
        };
        output.success(&call.invocation, message, Feedback::SourceAndOperators);
        Ok(count)
    })
}

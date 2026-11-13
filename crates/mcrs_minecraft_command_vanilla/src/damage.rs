use crate::mob_hit::HitMob;
use crate::select::{SelectionBuffers, select};
use crate::set::{VanillaCommands, VanillaContext};
use bevy_ecs::prelude::*;
use mcrs_command::{ArgumentHandle, BuildError, FloatArgument, NodeId};
use mcrs_minecraft_command::{CommandLevel, PermissionCheck};
use mcrs_minecraft_command_argument::ResourceArgument;
use mcrs_minecraft_command_argument_entity::EntityArgument;
use mcrs_minecraft_command_engine::{
    Call, CommandFailure, CommandOutput, CommandRegistrar, CommandResult, EffectQueue, Feedback,
};
use mcrs_minecraft_damage::{Defence, Hit, HitOutcome, preview};
use mcrs_minecraft_engine_world::birth_clock::BirthClock;
use mcrs_minecraft_physics::Living;
use mcrs_minecraft_resource::{ResourceLocation, rl};
use mcrs_minecraft_text::Text;

const DAMAGE_TYPE: ResourceLocation<'static> = rl!("minecraft:damage_type");

#[derive(Clone, Copy)]
pub(crate) struct DamageCommand {
    target: ArgumentHandle<EntityArgument>,
    amount: ArgumentHandle<FloatArgument>,
    damage_type: ArgumentHandle<ResourceArgument>,
}

pub(crate) fn build(
    registrar: &mut CommandRegistrar<'_, VanillaCommands, ()>,
) -> Result<DamageCommand, BuildError> {
    let damage = registrar
        .literal(NodeId::ROOT, "damage")
        .requires(PermissionCheck::Require(CommandLevel::Gamemasters))
        .id()?;
    let target = registrar
        .arg(damage, "target", EntityArgument::ONE_ENTITY)
        .id()?;
    let amount = registrar
        .arg(
            target,
            "amount",
            FloatArgument {
                min: 0.0,
                max: f32::MAX,
            },
        )
        .runs(())
        .id()?;
    let damage_type = registrar
        .arg(
            amount,
            "damageType",
            ResourceArgument {
                registry: DAMAGE_TYPE,
            },
        )
        .runs(())
        .id()?;
    Ok(DamageCommand {
        target,
        amount,
        damage_type,
    })
}

/// The network id of the damage type `name`: its position in the registry that the clients got.
/// A type the registry does not hold fails the command, since another id would hurt with an
/// unrelated type.
pub(crate) fn damage_type_id(
    context: &VanillaContext,
    name: ResourceLocation<'_>,
) -> Result<u32, CommandFailure> {
    context
        .lookup()
        .registry_lookup()
        .names(DAMAGE_TYPE)
        .iter()
        .position(|entry| *entry == name)
        .and_then(|index| u32::try_from(index).ok())
        .ok_or_else(CommandFailure::failed)
}

/// CMD-DMG-001 to CMD-DMG-004, for a mob target and a source with no entity and no position: one
/// hit of the amount and the damage type, `minecraft:generic` by default.
pub(crate) fn run(
    In(call): In<Call<(), DamageCommand>>,
    context: VanillaContext,
    mut output: CommandOutput,
    mut hits: ResMut<EffectQueue<HitMob>>,
    clock: Option<Res<BirthClock>>,
    mut found: Local<SelectionBuffers>,
) -> CommandResult {
    let command = call.command;
    let amount = context.arg(&call.invocation, command.amount)?;
    let damage_type = match context.opt(&call.invocation, command.damage_type)? {
        Some(name) => damage_type_id(&context, name)?,
        None => damage_type_id(&context, rl!("minecraft:generic"))?,
    };
    found.with(|targets| {
        select(&context, &call.invocation, command.target, targets)?;
        let view = targets.single_mob()?;
        let Some(living) = view.state.living.filter(Living::is_alive) else {
            return Err(CommandFailure::translate(
                "commands.damage.invulnerable",
                vec![],
            ));
        };
        // A hit that the cooldown rejects still resets the idle time (CMD-DMG-003), so it goes
        // to the row before the command fails.
        output.effect(
            &mut hits,
            HitMob {
                at: view.at,
                hit: Hit::new(amount, damage_type),
            },
        );
        let tick = clock.as_deref().map_or(0, |clock| clock.0.max(0) as u64);
        if preview(
            &living.vitals,
            &Defence::default(),
            Hit::new(amount, damage_type),
            tick,
        ) == HitOutcome::Rejected
        {
            return Err(CommandFailure::translate(
                "commands.damage.invulnerable",
                vec![],
            ));
        }
        output.success(
            &call.invocation,
            Text::translate_args(
                "commands.damage.success",
                vec![amount.into(), view.display_name().into()],
            ),
            Feedback::SourceAndOperators,
        );
        Ok(1)
    })
}

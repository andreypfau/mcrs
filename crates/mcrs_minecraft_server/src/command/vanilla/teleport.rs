use crate::command::vanilla::{select, targets_or_self};
use crate::command::{
    Call, CommandContext, CommandFailure, CommandOutput, CommandRegistrar, CommandResult, Facing,
    PlayerEntry, Teleport,
};
use crate::world::entity::player::movement::{MAX_XZ, MAX_Y};
use bevy_ecs::prelude::*;
use bevy_math::DVec3;
use mcrs_command::{Arg, BuildError, NodeId};
use mcrs_minecraft_command::{
    CommandLevel, EntityAnchorArgument, EntityArgument, PermissionCheck, RotationArgument,
    Vec3Argument,
};
use mcrs_minecraft_protocol::packets::game::clientbound::LookAnchor;
use mcrs_minecraft_protocol::text::{IntoText, Text};

#[derive(Clone, Copy)]
pub(super) struct TeleportCommand {
    location: Arg<Vec3Argument>,
    destination: Arg<EntityArgument>,
    targets: Arg<EntityArgument>,
    target_location: Arg<Vec3Argument>,
    rotation: Arg<RotationArgument>,
    facing_entity: Arg<EntityArgument>,
    facing_anchor: Arg<EntityAnchorArgument>,
    facing_location: Arg<Vec3Argument>,
    target_destination: Arg<EntityArgument>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Branch {
    ToLocation,
    ToEntity,
    TargetsToLocation,
    TargetsToLocationRotated,
    TargetsFacingEntity,
    TargetsFacingLocation,
    TargetsToEntity,
}

/// CMD-TP-001.
pub(super) fn build(r: &mut CommandRegistrar<'_, Branch>) -> Result<TeleportCommand, BuildError> {
    let teleport = r
        .literal(NodeId::ROOT, "teleport")
        .requires(PermissionCheck::Require(CommandLevel::Gamemasters))
        .id()?;
    let location = r
        .arg(teleport, "location", Vec3Argument)
        .runs(Branch::ToLocation)
        .id()?;
    let destination = r
        .arg(teleport, "destination", EntityArgument::ONE_ENTITY)
        .runs(Branch::ToEntity)
        .id()?;
    let targets = r.arg(teleport, "targets", EntityArgument::ENTITIES).id()?;
    let target_location = r
        .arg(targets, "location", Vec3Argument)
        .runs(Branch::TargetsToLocation)
        .id()?;
    let rotation = r
        .arg(target_location, "rotation", RotationArgument)
        .runs(Branch::TargetsToLocationRotated)
        .id()?;
    let facing = r.literal(target_location, "facing").id()?;
    let entity = r.literal(facing, "entity").id()?;
    let facing_entity = r
        .arg(entity, "facingEntity", EntityArgument::ONE_ENTITY)
        .runs(Branch::TargetsFacingEntity)
        .id()?;
    let facing_anchor = r
        .arg(facing_entity, "facingAnchor", EntityAnchorArgument)
        .runs(Branch::TargetsFacingEntity)
        .id()?;
    let facing_location = r
        .arg(facing, "facingLocation", Vec3Argument)
        .runs(Branch::TargetsFacingLocation)
        .id()?;
    let target_destination = r
        .arg(targets, "destination", EntityArgument::ONE_ENTITY)
        .runs(Branch::TargetsToEntity)
        .id()?;
    r.literal(NodeId::ROOT, "tp")
        .requires(PermissionCheck::Require(CommandLevel::Gamemasters))
        .redirect(teleport)
        .id()?;
    Ok(TeleportCommand {
        location,
        destination,
        targets,
        target_location,
        rotation,
        facing_entity,
        facing_anchor,
        facing_location,
        target_destination,
    })
}

fn spawnable(position: DVec3) -> bool {
    let block = position.floor();
    (-MAX_XZ..MAX_XZ).contains(&block.x)
        && (-MAX_XZ..MAX_XZ).contains(&block.z)
        && (-MAX_Y..MAX_Y).contains(&block.y)
}

/// Where each victim goes (CMD-TP-004), and the name of the entity the victims go to.
struct Destination {
    to: Teleport,
    to_entity: Option<Text>,
}

/// A single entity argument finds at most one entity.
fn one(found: Vec<&PlayerEntry>) -> Result<&PlayerEntry, CommandFailure> {
    found
        .first()
        .copied()
        .ok_or_else(|| CommandFailure::translate("argument.entity.notfound.entity", vec![]))
}

fn to_entity(
    context: &CommandContext,
    call: &Call<Branch, TeleportCommand>,
    arg: Arg<EntityArgument>,
) -> Result<Destination, CommandFailure> {
    let entity = one(select(context, call, arg)?)?;
    let dimension = context
        .targets()
        .label_of(entity.dimension)
        .ok_or_else(CommandFailure::failed)?;
    Ok(Destination {
        to: Teleport {
            dimension,
            position: entity.transform.translation,
            rotation: Some((
                entity.transform.rotation.yaw(),
                entity.transform.rotation.pitch(),
            )),
            relative_position: [false; 3],
            relative_rotation: [false; 2],
            facing: None,
            anchor: call.source.anchor,
        },
        to_entity: Some(entity.name.clone().into_text()),
    })
}

/// Where the branch of the call sends each victim (CMD-TP-004).
fn destination(
    command: &TeleportCommand,
    context: &CommandContext,
    call: &Call<Branch, TeleportCommand>,
) -> Result<Destination, CommandFailure> {
    let source = context.source(call);
    let (location, rotation, facing) = match call.branch {
        Branch::ToEntity => return to_entity(context, call, command.destination),
        Branch::TargetsToEntity => return to_entity(context, call, command.target_destination),
        Branch::ToLocation => (context.arg(call, command.location)?, None, None),
        Branch::TargetsToLocation => (context.arg(call, command.target_location)?, None, None),
        Branch::TargetsToLocationRotated => (
            context.arg(call, command.target_location)?,
            Some(context.arg(call, command.rotation)?),
            None,
        ),
        Branch::TargetsFacingEntity => {
            let entity = one(select(context, call, command.facing_entity)?)?;
            let at: LookAnchor = context
                .opt(call, command.facing_anchor)?
                .unwrap_or_default();
            (
                context.arg(call, command.target_location)?,
                None,
                Some(Facing::Player {
                    anchor: entity.anchor,
                    at,
                }),
            )
        }
        Branch::TargetsFacingLocation => (
            context.arg(call, command.target_location)?,
            None,
            Some(Facing::Position(
                context
                    .arg(call, command.facing_location)?
                    .position(&source),
            )),
        ),
    };
    Ok(Destination {
        to: Teleport {
            dimension: call.source.dimension,
            position: location.position(&source),
            rotation: rotation.map(|rotation| rotation.resolve(&source)),
            relative_position: location.relative(),
            relative_rotation: rotation.map_or([true; 2], |r| [r.yaw.relative, r.pitch.relative]),
            facing,
            anchor: source.anchor,
        },
        to_entity: None,
    })
}

fn coordinate(value: f64) -> Text {
    format!("{value:.6}").into_text()
}

/// CMD-TP-003 to CMD-TP-007, CMD-TP-013 to CMD-TP-015. The victims move one after another in
/// the order of the selector, each in the world that holds it, and the message follows the
/// last move. A victim the search found is still where the search found it when its move
/// applies, so each victim moves and counts.
pub(super) fn run(
    In(call): In<Call<Branch, TeleportCommand>>,
    context: CommandContext,
    mut output: CommandOutput,
) -> CommandResult {
    let command = call.command;
    let selector =
        (!matches!(call.branch, Branch::ToLocation | Branch::ToEntity)).then_some(command.targets);
    let victims = targets_or_self(&context, &call, selector, "argument.entity.notfound.entity")?;
    let destination = destination(&command, &context, &call)?;
    if !spawnable(destination.to.position) {
        return Err(CommandFailure::translate(
            "commands.teleport.invalidPosition",
            vec![],
        ));
    }
    let Some(first) = victims.first() else {
        return Ok(0);
    };
    for victim in &victims {
        output.teleport(victim.anchor, destination.to);
    }
    let moved = victims.len();
    let single = moved == 1;
    let subject = if single {
        first.name.clone().into_text()
    } else {
        moved.to_string().into_text()
    };
    let position = destination.to.position;
    let message = match destination.to_entity {
        Some(entity) => Text::translate(
            if single {
                "commands.teleport.success.entity.single"
            } else {
                "commands.teleport.success.entity.multiple"
            },
            vec![subject, entity],
        ),
        None => Text::translate(
            if single {
                "commands.teleport.success.location.single"
            } else {
                "commands.teleport.success.location.multiple"
            },
            vec![
                subject,
                coordinate(position.x),
                coordinate(position.y),
                coordinate(position.z),
            ],
        ),
    };
    output.success(&call, message, true);
    Ok(i32::try_from(moved).unwrap_or(i32::MAX))
}

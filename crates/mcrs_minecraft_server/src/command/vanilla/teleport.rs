use crate::command::vanilla::{PlayerCandidate, candidates, select, targets_or_self};
use crate::command::{
    Call, CommandContext, CommandFailure, CommandFeedback, CommandRegistrar, CommandResult,
};
use crate::world::entity::player::HostAnchor;
use crate::world::entity::player::ability::PlayerGameMode;
use crate::world::entity::player::movement::{MAX_XZ, MAX_Y, TeleportState};
use bevy_ecs::prelude::*;
use bevy_math::DVec3;
use mcrs_command::{Arg, BuildError, NodeId};
use mcrs_minecraft_command::teleport::{Pose, look_at, relative_move};
use mcrs_minecraft_command::{
    CommandLevel, EntityAnchorArgument, EntityArgument, PermissionCheck, RotationArgument,
    Vec3Argument, anchored,
};
use mcrs_minecraft_engine::bus::{PacketPayload, to as packet_to};
use mcrs_minecraft_engine::session::GameProfile;
use mcrs_minecraft_entity::PLAYER_EYE_HEIGHT;
use mcrs_minecraft_level::entity::physics::{Rotation, Transform};
use mcrs_minecraft_level::entity::player::Player;
use mcrs_minecraft_protocol::packets::game::clientbound::{
    ClientboundPlayerLookAt, ClientboundPlayerPosition, LookAnchor,
};
use mcrs_minecraft_protocol::text::{IntoText, Text};
use mcrs_minecraft_protocol::{PositionFlags, VarInt};

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

/// Where each victim goes (CMD-TP-004).
struct Destination {
    position: DVec3,
    /// `None` keeps the rotation of the victim.
    rotation: Option<(f32, f32)>,
    relative_position: [bool; 3],
    relative_rotation: [bool; 2],
    facing: Option<Facing>,
    /// The name of the entity the victims go to.
    to_entity: Option<Text>,
}

/// What each victim turns to after its move (CMD-TP-011).
struct Facing {
    target: DVec3,
    /// The network id of `facingEntity` and its anchor.
    entity: Option<(VarInt, LookAnchor)>,
}

/// A single entity argument finds at most one entity.
fn one<'c, 'n>(
    found: Vec<&'c PlayerCandidate<'n>>,
) -> Result<&'c PlayerCandidate<'n>, CommandFailure> {
    found
        .first()
        .copied()
        .ok_or_else(|| CommandFailure::translate("argument.entity.notfound.entity", vec![]))
}

fn to_entity(
    context: &CommandContext,
    call: &Call<Branch, TeleportCommand>,
    arg: Arg<EntityArgument>,
    all: &[PlayerCandidate<'_>],
) -> Result<Destination, CommandFailure> {
    let entity = one(select(context, call, arg, all)?)?;
    Ok(Destination {
        position: entity.transform.translation,
        rotation: Some((
            entity.transform.rotation.yaw(),
            entity.transform.rotation.pitch(),
        )),
        relative_position: [false; 3],
        relative_rotation: [false; 2],
        facing: None,
        to_entity: Some(entity.name.to_owned().into_text()),
    })
}

/// Where the branch of the call sends each victim (CMD-TP-004).
fn destination(
    command: &TeleportCommand,
    context: &CommandContext,
    call: &Call<Branch, TeleportCommand>,
    all: &[PlayerCandidate<'_>],
) -> Result<Destination, CommandFailure> {
    let source = context.source(call);
    let (location, rotation, facing) = match call.branch {
        Branch::ToEntity => return to_entity(context, call, command.destination, all),
        Branch::TargetsToEntity => {
            return to_entity(context, call, command.target_destination, all);
        }
        Branch::ToLocation => (context.arg(call, command.location)?, None, None),
        Branch::TargetsToLocation => (context.arg(call, command.target_location)?, None, None),
        Branch::TargetsToLocationRotated => (
            context.arg(call, command.target_location)?,
            Some(context.arg(call, command.rotation)?),
            None,
        ),
        Branch::TargetsFacingEntity => {
            let entity = one(select(context, call, command.facing_entity, all)?)?;
            let anchor: LookAnchor = context
                .opt(call, command.facing_anchor)?
                .unwrap_or_default();
            (
                context.arg(call, command.target_location)?,
                None,
                Some(Facing {
                    target: anchored(anchor, entity.transform.translation, PLAYER_EYE_HEIGHT),
                    entity: Some((VarInt(entity.entity.index_u32() as i32), anchor)),
                }),
            )
        }
        Branch::TargetsFacingLocation => (
            context.arg(call, command.target_location)?,
            None,
            Some(Facing {
                target: context
                    .arg(call, command.facing_location)?
                    .position(&source),
                entity: None,
            }),
        ),
    };
    Ok(Destination {
        position: location.position(&source),
        rotation: rotation.map(|rotation| rotation.resolve(&source)),
        relative_position: location.relative(),
        relative_rotation: rotation.map_or([true; 2], |r| [r.yaw.relative, r.pitch.relative]),
        facing,
        to_entity: None,
    })
}

/// Moves one victim, and gives the packets that tell its player (CMD-TP-005, CMD-TP-011,
/// CMD-TP-012).
fn move_victim(
    to: &Destination,
    anchor: LookAnchor,
    transform: &mut Transform,
    state: &mut TeleportState,
) -> (ClientboundPlayerPosition, Option<ClientboundPlayerLookAt>) {
    let from = Pose {
        position: transform.translation,
        yaw: transform.rotation.yaw(),
        pitch: transform.rotation.pitch(),
    };
    let (yaw, pitch) = to.rotation.unwrap_or((from.yaw, from.pitch));
    let destination = Pose {
        position: to.position,
        yaw,
        pitch,
    };
    let step = relative_move(
        from,
        destination,
        to.relative_position,
        to.relative_rotation,
    );
    let (yaw, pitch) = match &to.facing {
        Some(facing) => look_at(
            anchored(anchor, to.position, PLAYER_EYE_HEIGHT),
            facing.target,
        ),
        None => (step.result.yaw, step.result.pitch),
    };
    *transform = Transform {
        translation: to.position,
        rotation: Rotation::new(yaw, pitch),
    };
    let position = ClientboundPlayerPosition {
        teleport_id: VarInt(state.sent(*transform)),
        position: step.position,
        velocity: DVec3::ZERO,
        look: step.look,
        flags: PositionFlags(step.flags),
    };
    let look_at = to.facing.as_ref().map(|facing| ClientboundPlayerLookAt {
        from: anchor,
        target: facing.target,
        entity: facing.entity,
    });
    (position, look_at)
}

fn coordinate(value: f64) -> Text {
    format!("{value:.6}").into_text()
}

pub(super) fn run(
    In(call): In<Call<Branch, TeleportCommand>>,
    context: CommandContext,
    mut players: Query<
        (
            Entity,
            &GameProfile,
            &mut Transform,
            &PlayerGameMode,
            &mut TeleportState,
            &HostAnchor,
        ),
        With<Player>,
    >,
    mut feedback: CommandFeedback,
) -> CommandResult {
    let command = call.command;
    let all = candidates(
        players
            .iter()
            .map(|(entity, profile, transform, mode, _, _)| (entity, profile, transform, mode)),
    );
    let selector =
        (!matches!(call.branch, Branch::ToLocation | Branch::ToEntity)).then_some(command.targets);
    let victims = targets_or_self(
        &context,
        &call,
        selector,
        &all,
        "argument.entity.notfound.entity",
    )?;
    let to = destination(&command, &context, &call, &all)?;
    if !spawnable(to.position) {
        return Err(CommandFailure::translate(
            "commands.teleport.invalidPosition",
            vec![],
        ));
    }
    let anchor = context.source(&call).anchor;
    let mut moved = 0usize;
    let mut first = None;
    for victim in victims {
        let Ok((_, profile, mut transform, _, mut state, &HostAnchor(host))) =
            players.get_mut(victim)
        else {
            continue;
        };
        let (position, look_at) = move_victim(&to, anchor, &mut transform, &mut state);
        feedback.send(packet_to(host, PacketPayload::PlayerPosition(position)).critical());
        if let Some(look_at) = look_at {
            feedback.send(packet_to(host, PacketPayload::PlayerLookAt(look_at)).critical());
        }
        first.get_or_insert_with(|| profile.username.clone());
        moved += 1;
    }
    let Some(first) = first else {
        return Ok(0);
    };
    let single = moved == 1;
    let subject = if single {
        first.into_text()
    } else {
        moved.to_string().into_text()
    };
    let message = match &to.to_entity {
        Some(entity) => Text::translate(
            if single {
                "commands.teleport.success.entity.single"
            } else {
                "commands.teleport.success.entity.multiple"
            },
            vec![subject, entity.clone()],
        ),
        None => Text::translate(
            if single {
                "commands.teleport.success.location.single"
            } else {
                "commands.teleport.success.location.multiple"
            },
            vec![
                subject,
                coordinate(to.position.x),
                coordinate(to.position.y),
                coordinate(to.position.z),
            ],
        ),
    };
    feedback.success(&call, message, true);
    Ok(i32::try_from(moved).unwrap_or(i32::MAX))
}

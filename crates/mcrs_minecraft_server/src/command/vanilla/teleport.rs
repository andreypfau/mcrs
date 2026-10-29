use crate::command::Invocation;
use crate::command::vanilla::{FoundPlayers, select, targets_or_self};
use crate::command::{
    Call, CommandContext, CommandFailure, CommandOutput, CommandRegistrar, CommandResult,
    EffectQueue, PlayerEntry, Worlds,
};
use crate::world::entity::player::dimension_change::Arrival;
use crate::world::entity::player::movement::{MAX_XZ, MAX_Y, TeleportState};
use crate::world::entity::player_network_id;
use bevy_ecs::prelude::*;
use bevy_math::DVec3;
use mcrs_command::{ArgumentHandle, BuildError, NodeId};
use mcrs_minecraft_command::anchored;
use mcrs_minecraft_command::{CommandLevel, EntityAnchorArgument, PermissionCheck};
use mcrs_minecraft_command_coordinates::rotation_toward;
use mcrs_minecraft_command_coordinates::{Pose, relative_move};
use mcrs_minecraft_command_coordinates::{RotationArgument, Vec3Argument};
use mcrs_minecraft_command_entity::EntityArgument;
use mcrs_minecraft_engine::bus::{PacketPayload, to as to_player};
use mcrs_minecraft_entity::PLAYER_EYE_HEIGHT;
use mcrs_minecraft_level::entity::physics::{Rotation, Transform};
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundPlayerLookAt;
use mcrs_minecraft_protocol::packets::game::clientbound::LookAnchor;
use mcrs_minecraft_protocol::text::{IntoText, Text};

/// Where a teleport sends one victim (CMD-TP-004, CMD-TP-005).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Teleport {
    /// The label of the destination dimension.
    pub dimension: Entity,
    pub position: DVec3,
    /// `None` keeps the rotation of the victim.
    pub rotation: Option<(f32, f32)>,
    pub relative_position: [bool; 3],
    pub relative_rotation: [bool; 2],
    pub facing: Option<Facing>,
    /// The anchor of the source, from which the victim looks at its facing target.
    pub anchor: LookAnchor,
}

/// What a victim turns to after its move (CMD-TP-011).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Facing {
    Position(DVec3),
    /// The player with this host anchor, at its feet or its eyes, read after each move.
    Player {
        anchor: Entity,
        at: LookAnchor,
    },
}

/// The command effect that moves the player with the host anchor `player`, in any dimension.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TeleportPlayer {
    pub player: Entity,
    pub to: Teleport,
}

#[derive(Clone, Copy)]
pub(super) struct TeleportCommand {
    location: ArgumentHandle<Vec3Argument>,
    destination: ArgumentHandle<EntityArgument>,
    targets: ArgumentHandle<EntityArgument>,
    target_location: ArgumentHandle<Vec3Argument>,
    rotation: ArgumentHandle<RotationArgument>,
    facing_entity: ArgumentHandle<EntityArgument>,
    facing_anchor: ArgumentHandle<EntityAnchorArgument>,
    facing_location: ArgumentHandle<Vec3Argument>,
    target_destination: ArgumentHandle<EntityArgument>,
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

/// The first player a selector argument finds.
fn one<'c>(
    context: &'c CommandContext,
    call: &Invocation,
    arg: ArgumentHandle<EntityArgument>,
    found: &mut FoundPlayers,
) -> Result<&'c PlayerEntry, CommandFailure> {
    let first = found.with(|out| {
        select(context, call, arg, out)?;
        Ok::<_, CommandFailure>(out.first().copied())
    })?;
    first.ok_or_else(|| CommandFailure::translate("argument.entity.notfound.entity", vec![]))
}

fn to_entity(
    context: &CommandContext,
    call: &Call<Branch, TeleportCommand>,
    arg: ArgumentHandle<EntityArgument>,
    found: &mut FoundPlayers,
) -> Result<Destination, CommandFailure> {
    let entity = one(context, call, arg, found)?;
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
    found: &mut FoundPlayers,
) -> Result<Destination, CommandFailure> {
    let source = context.source(call)?;
    let (location, rotation, facing) = match call.branch {
        Branch::ToEntity => return to_entity(context, call, command.destination, found),
        Branch::TargetsToEntity => {
            return to_entity(context, call, command.target_destination, found);
        }
        Branch::ToLocation => (context.arg(call, command.location)?, None, None),
        Branch::TargetsToLocation => (context.arg(call, command.target_location)?, None, None),
        Branch::TargetsToLocationRotated => (
            context.arg(call, command.target_location)?,
            Some(context.arg(call, command.rotation)?),
            None,
        ),
        Branch::TargetsFacingEntity => {
            let entity = one(context, call, command.facing_entity, found)?;
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
    mut teleports: ResMut<EffectQueue<TeleportPlayer>>,
    mut found_victims: Local<FoundPlayers>,
    mut found_entity: Local<FoundPlayers>,
) -> CommandResult {
    let command = call.command;
    found_victims.with(|victims| {
        targets_or_self(
            &context,
            &call,
            command.targets,
            "argument.entity.notfound.entity",
            victims,
        )?;
        let destination = destination(&command, &context, &call, &mut found_entity)?;
        if !spawnable(destination.to.position) {
            return Err(CommandFailure::translate(
                "commands.teleport.invalidPosition",
                vec![],
            ));
        }
        let Some(&first) = victims.first() else {
            return Ok(0);
        };
        for victim in victims.iter() {
            output.effect(
                &mut teleports,
                TeleportPlayer {
                    player: victim.anchor,
                    to: destination.to,
                },
            );
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
    })
}

/// The position a victim turns to, and the entity of `player_look_at` (CMD-TP-011,
/// CMD-TP-012), read after the moves before it. A victim that faces itself faces its
/// destination `at`.
fn facing(
    worlds: &Worlds,
    victim: Entity,
    at: DVec3,
    facing: Facing,
) -> Option<(DVec3, Option<(VarInt, LookAnchor)>)> {
    match facing {
        Facing::Position(position) => Some((position, None)),
        Facing::Player { anchor, at: part } => {
            let position = if anchor == victim {
                at
            } else {
                worlds.targets().player(anchor)?.transform.translation
            };
            Some((
                anchored(part, position, PLAYER_EYE_HEIGHT),
                Some((VarInt(player_network_id(anchor)), part)),
            ))
        }
    }
}

/// Moves one victim (CMD-TP-005, CMD-TP-013); to another dimension as DIM-CHG states.
pub(crate) fn apply(worlds: &mut Worlds, TeleportPlayer { player, to }: TeleportPlayer) {
    let targets = worlds.targets();
    let (Some(victim), Some(destination)) =
        (targets.player(player), targets.dimension_of(to.dimension))
    else {
        tracing::error!(?player, "a teleport names no online player or no dimension");
        return;
    };
    let (origin, current) = (victim.dimension, victim.transform);
    let from = Pose {
        position: current.translation,
        yaw: current.rotation.yaw(),
        pitch: current.rotation.pitch(),
    };
    let (yaw, pitch) = to.rotation.unwrap_or((from.yaw, from.pitch));
    let destination_pose = Pose {
        position: to.position,
        yaw,
        pitch,
    };
    let step = relative_move(
        from,
        destination_pose,
        to.relative_position,
        to.relative_rotation,
        origin != destination,
    );
    let mut transform = Transform {
        translation: to.position,
        rotation: Rotation::new(step.final_look.yaw, step.final_look.pitch),
    };
    let facing = to
        .facing
        .and_then(|target| facing(worlds, player, to.position, target));
    if let Some((target, _)) = facing {
        let eyes = anchored(to.anchor, to.position, PLAYER_EYE_HEIGHT);
        let (yaw, pitch) = rotation_toward(eyes, target);
        transform.rotation = Rotation::new(yaw, pitch);
    }
    let arrival = Arrival {
        transform,
        position: step.position,
        look: step.look,
        flags: step.flags,
    };
    if origin == destination {
        let entity = match worlds.set_transform(player, transform) {
            Ok(entity) => entity,
            Err(error) => {
                tracing::error!(?player, "a teleport did not move its victim: {error}");
                return;
            }
        };
        let teleport_id = worlds
            .world_mut(origin)
            .and_then(|world| world.get_mut::<TeleportState>(entity))
            .map(|mut state| state.sent(transform));
        if let Some(teleport_id) = teleport_id {
            let position = arrival.position_packet(teleport_id);
            worlds.deliver(to_player(player, PacketPayload::PlayerPosition(position)).critical());
        }
    } else if let Err(error) = worlds.change_dimension(player, destination, arrival) {
        tracing::error!(?player, "a teleport did not move its victim: {error}");
        return;
    }
    if let Some((target, entity)) = facing {
        let look_at = ClientboundPlayerLookAt {
            from: to.anchor,
            target,
            entity,
        };
        worlds.deliver(to_player(player, PacketPayload::PlayerLookAt(look_at)).critical());
    }
}

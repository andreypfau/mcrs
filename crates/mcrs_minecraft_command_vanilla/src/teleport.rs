use crate::entity_view::EntityView;
use crate::relative_move::{DimensionChange, Pose, RelativeRotation, relative_move};
use crate::select::{SelectionBuffers, SelfSource, select, targets_or_self};
use crate::set::{VanillaCommands, VanillaContext};
use bevy_ecs::prelude::*;
use bevy_math::{BVec3, DVec3};
use mcrs_command::{ArgumentHandle, BuildError, NodeId};
use mcrs_minecraft_command::{CommandLevel, PermissionCheck};
use mcrs_minecraft_command_argument::EntityAnchorArgument;
use mcrs_minecraft_command_argument::{RotationArgument, Vec3Argument};
use mcrs_minecraft_command_argument_entity::EntityArgument;
use mcrs_minecraft_command_engine::{
    Call, CommandFailure, CommandOutput, CommandRegistrar, CommandResult, EffectQueue, Feedback,
    Invocation, PlayerEntry, Worlds,
};
use mcrs_minecraft_core::Look;
use mcrs_minecraft_core::LookAnchor;
use mcrs_minecraft_engine::dimension_change::Arrival;
use mcrs_minecraft_engine::network_id;
use mcrs_minecraft_engine::teleport::{MAX_XZ, MAX_Y};
use mcrs_minecraft_engine_world::entity::physics::{Rotation, Transform};
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_entity::PLAYER_EYE_HEIGHT;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol_game::clientbound::{ClientboundPlayerLookAt, LookAtEntity};
use mcrs_minecraft_text::{IntoText, Text};

/// Where a teleport sends one victim.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Teleport {
    /// The label of the destination dimension.
    pub dimension: Entity,
    /// The destination, with each relative axis already resolved against the source.
    pub position: DVec3,
    /// `None` keeps the rotation of the victim.
    pub rotation: Option<Look>,
    /// The axes of the position that the client receives as a change.
    pub relative_position: BVec3,
    /// The angles of the rotation that the client receives as a change.
    pub relative_rotation: RelativeRotation,
    /// What the victim turns to after the move.
    pub facing: Option<Facing>,
    /// The anchor of the source, from which the victim looks at its facing target.
    pub anchor: LookAnchor,
}

/// What a victim turns to after its move.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Facing {
    /// A fixed position.
    Position(DVec3),
    /// The player of this session, at its feet or its eyes, read after each move.
    Player {
        /// The session of the player.
        player: PlayerSession,
        /// The part of the player to face.
        at: LookAnchor,
    },
}

/// The command effect that moves the player of the session `player`, in any dimension.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TeleportPlayer {
    /// The session of the victim.
    pub player: PlayerSession,
    /// Where the victim goes.
    pub to: Teleport,
}

#[derive(Clone, Copy)]
pub(crate) struct TeleportCommand {
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
pub(crate) enum Branch {
    ToLocation,
    ToEntity,
    TargetsToLocation,
    TargetsToLocationRotated,
    TargetsFacingEntity,
    TargetsFacingLocation,
    TargetsToEntity,
}

pub(crate) fn build(
    registrar: &mut CommandRegistrar<'_, VanillaCommands, Branch>,
) -> Result<TeleportCommand, BuildError> {
    let teleport = registrar
        .literal(NodeId::ROOT, "teleport")
        .requires(PermissionCheck::Require(CommandLevel::Gamemasters))
        .id()?;
    let location = registrar
        .arg(teleport, "location", Vec3Argument)
        .runs(Branch::ToLocation)
        .in_any_world()
        .id()?;
    let destination = registrar
        .arg(teleport, "destination", EntityArgument::ONE_ENTITY)
        .runs(Branch::ToEntity)
        .in_any_world()
        .id()?;
    let targets = registrar
        .arg(teleport, "targets", EntityArgument::ENTITIES)
        .id()?;
    let target_location = registrar
        .arg(targets, "location", Vec3Argument)
        .runs(Branch::TargetsToLocation)
        .in_any_world()
        .id()?;
    let rotation = registrar
        .arg(target_location, "rotation", RotationArgument)
        .runs(Branch::TargetsToLocationRotated)
        .in_any_world()
        .id()?;
    let facing = registrar.literal(target_location, "facing").id()?;
    let entity = registrar.literal(facing, "entity").id()?;
    let facing_entity = registrar
        .arg(entity, "facingEntity", EntityArgument::ONE_ENTITY)
        .runs(Branch::TargetsFacingEntity)
        .in_any_world()
        .id()?;
    let facing_anchor = registrar
        .arg(facing_entity, "facingAnchor", EntityAnchorArgument)
        .runs(Branch::TargetsFacingEntity)
        .in_any_world()
        .id()?;
    let facing_location = registrar
        .arg(facing, "facingLocation", Vec3Argument)
        .runs(Branch::TargetsFacingLocation)
        .in_any_world()
        .id()?;
    let target_destination = registrar
        .arg(targets, "destination", EntityArgument::ONE_ENTITY)
        .runs(Branch::TargetsToEntity)
        .in_any_world()
        .id()?;
    registrar
        .literal(NodeId::ROOT, "tp")
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

/// Where each victim goes, and the name of the entity the victims go to.
struct Destination {
    to: Teleport,
    to_entity: Option<Text>,
}

/// The first entity a selector argument finds.
fn one<'c>(
    context: &'c VanillaContext,
    call: &Invocation,
    arg: ArgumentHandle<EntityArgument>,
    found: &mut SelectionBuffers,
) -> Result<EntityView<'c>, CommandFailure> {
    let first = found.with(|out| {
        select(context, call, arg, out)?;
        Ok::<_, CommandFailure>(out.first())
    })?;
    first.ok_or_else(|| CommandFailure::translate("argument.entity.notfound.entity", vec![]))
}

/// The failure of a teleport that would move a mob or face one: the physics rows have no move
/// of a mob to an arbitrary position yet, and a silent skip would report a move that did not
/// happen.
fn mob_not_supported() -> CommandFailure {
    CommandFailure(Text::text(
        "teleport does not move or face a mob yet; only players can be victims or facing targets",
    ))
}

fn to_entity(
    context: &VanillaContext,
    call: &Call<Branch, TeleportCommand>,
    arg: ArgumentHandle<EntityArgument>,
    found: &mut SelectionBuffers,
) -> Result<Destination, CommandFailure> {
    let entity = one(context, &call.invocation, arg, found)?;
    let dimension = entity.dimension_label(context.targets(), call.invocation.source.dimension);
    Ok(Destination {
        to: Teleport {
            dimension,
            position: entity.position(),
            rotation: Some(entity.look()),
            relative_position: BVec3::FALSE,
            relative_rotation: RelativeRotation {
                yaw: false,
                pitch: false,
            },
            facing: None,
            anchor: call.invocation.source.anchor,
        },
        to_entity: Some(entity.display_name()),
    })
}

/// Where the branch of the call sends each victim.
fn destination(
    command: &TeleportCommand,
    context: &VanillaContext,
    call: &Call<Branch, TeleportCommand>,
    found: &mut SelectionBuffers,
) -> Result<Destination, CommandFailure> {
    let source = context.source(&call.invocation)?;
    let (location, rotation, facing) = match call.branch {
        Branch::ToEntity => return to_entity(context, call, command.destination, found),
        Branch::TargetsToEntity => {
            return to_entity(context, call, command.target_destination, found);
        }
        Branch::ToLocation => (context.arg(&call.invocation, command.location)?, None, None),
        Branch::TargetsToLocation => (
            context.arg(&call.invocation, command.target_location)?,
            None,
            None,
        ),
        Branch::TargetsToLocationRotated => (
            context.arg(&call.invocation, command.target_location)?,
            Some(context.arg(&call.invocation, command.rotation)?),
            None,
        ),
        Branch::TargetsFacingEntity => {
            let EntityView::Player(entity) =
                one(context, &call.invocation, command.facing_entity, found)?
            else {
                return Err(mob_not_supported());
            };
            let at: LookAnchor = context
                .opt(&call.invocation, command.facing_anchor)?
                .unwrap_or_default();
            (
                context.arg(&call.invocation, command.target_location)?,
                None,
                Some(Facing::Player {
                    player: entity.session,
                    at,
                }),
            )
        }
        Branch::TargetsFacingLocation => (
            context.arg(&call.invocation, command.target_location)?,
            None,
            Some(Facing::Position(
                context
                    .arg(&call.invocation, command.facing_location)?
                    .position(&source),
            )),
        ),
    };
    Ok(Destination {
        to: Teleport {
            dimension: call.invocation.source.dimension,
            position: location.position(&source),
            rotation: rotation.map(|rotation| rotation.resolve(&source)),
            relative_position: location.relative(),
            relative_rotation: rotation.map_or(
                RelativeRotation {
                    yaw: true,
                    pitch: true,
                },
                |rotation| RelativeRotation {
                    yaw: rotation.yaw.relative,
                    pitch: rotation.pitch.relative,
                },
            ),
            facing,
            anchor: source.anchor,
        },
        to_entity: None,
    })
}

fn coordinate(value: f64) -> Text {
    format!("{value:.6}").into_text()
}

/// The victims move one after another in the order of the selector, each in the world that
/// holds it, and the message follows the last move. A victim the search found is still where
/// the search found it when its move applies, so each victim moves and counts.
pub(crate) fn run(
    In(call): In<Call<Branch, TeleportCommand>>,
    context: VanillaContext,
    mut output: CommandOutput,
    mut teleports: ResMut<EffectQueue<TeleportPlayer>>,
    mut found_victims: Local<SelectionBuffers>,
    mut found_entity: Local<SelectionBuffers>,
) -> CommandResult {
    let command = call.command;
    found_victims.with(|victims| {
        targets_or_self(
            &context,
            &call.invocation,
            command.targets,
            "argument.entity.notfound.entity",
            SelfSource::Entity,
            victims,
        )?;
        if victims.mobs().next().is_some() {
            return Err(mob_not_supported());
        }
        let destination = destination(&command, &context, &call, &mut found_entity)?;
        if !spawnable(destination.to.position) {
            return Err(CommandFailure::translate(
                "commands.teleport.invalidPosition",
                vec![],
            ));
        }
        let Some(first) = victims.players().next() else {
            return Ok(0);
        };
        for victim in victims.players() {
            output.effect(
                &mut teleports,
                TeleportPlayer {
                    player: victim.session,
                    to: destination.to,
                },
            );
        }
        let moved = victims.len();
        let message = success_message(first, moved, destination);
        output.success(&call.invocation, message, Feedback::SourceAndOperators);
        Ok(i32::try_from(moved).unwrap_or(i32::MAX))
    })
}

/// The message of a teleport of `moved` victims, the first of which is `first`.
fn success_message(first: &PlayerEntry, moved: usize, destination: Destination) -> Text {
    let single = moved == 1;
    let subject = if single {
        first.name.clone().into_text()
    } else {
        moved.to_string().into_text()
    };
    let position = destination.to.position;
    match destination.to_entity {
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
    }
}

/// The position a victim turns to, and the entity of `player_look_at`, read after the moves
/// before it. A victim that faces itself faces its destination `at`.
fn facing(
    worlds: &Worlds,
    victim: PlayerSession,
    at: DVec3,
    facing: Facing,
) -> Option<(DVec3, Option<LookAtEntity>)> {
    match facing {
        Facing::Position(position) => Some((position, None)),
        Facing::Player { player, at: part } => {
            let position = if player == victim {
                at
            } else {
                worlds.targets().player(player)?.transform.translation
            };
            Some((
                mcrs_minecraft_command::anchored(part, position, PLAYER_EYE_HEIGHT),
                Some(LookAtEntity {
                    id: VarInt(network_id::player_network_id(player)),
                    anchor: part,
                }),
            ))
        }
    }
}

/// Moves one victim, to another dimension when the destination holds a different one.
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
        look: current.rotation.into(),
    };
    let destination_pose = Pose {
        position: to.position,
        look: to.rotation.unwrap_or(from.look),
    };
    let dimension_change = if origin == destination {
        DimensionChange::Stays
    } else {
        DimensionChange::Changes
    };
    let step = relative_move(
        from,
        destination_pose,
        to.relative_position,
        to.relative_rotation,
        dimension_change,
    );
    let mut transform = Transform {
        translation: to.position,
        rotation: Rotation::new(step.final_look.yaw, step.final_look.pitch),
    };
    let facing = to
        .facing
        .and_then(|target| facing(worlds, player, to.position, target));
    if let Some((target, _)) = facing {
        let eyes = mcrs_minecraft_command::anchored(to.anchor, to.position, PLAYER_EYE_HEIGHT);
        let look = mcrs_minecraft_command_argument::rotation_toward(eyes, target);
        transform.rotation = Rotation::new(look.yaw, look.pitch);
    }
    let arrival = Arrival {
        transform,
        position: step.position,
        look: step.look,
        flags: step.flags,
        look_at: facing.map(|(target, entity)| ClientboundPlayerLookAt {
            from: to.anchor,
            target,
            entity,
        }),
    };
    if let Err(error) = worlds.teleport(player, destination, arrival) {
        tracing::error!(?player, "a teleport did not move its victim: {error}");
    }
}

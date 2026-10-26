use crate::command::vanilla::{PlayerCandidate, candidates, name, select};
use crate::command::{
    AppCommands, CommandContext, CommandFailure, CommandFeedback, CommandRegistrar, CommandResult,
    CommandTree, DimLookup, Invocation,
};
use crate::world::entity::player::HostAnchor;
use crate::world::entity::player::ability::PlayerGameMode;
use crate::world::entity::player::movement::TeleportState;
use bevy_app::App;
use bevy_ecs::prelude::*;
use bevy_math::DVec3;
use mcrs_command::{Arg, BuildError};
use mcrs_minecraft_command::argument::coordinates::Coordinates;
use mcrs_minecraft_command::{
    Anchor, CommandLevel, EntityAnchorArgument, EntityArgument, PermissionCheck, RotationArgument,
    Source, Vec3Argument,
};
use mcrs_minecraft_core::mth::wrap_degrees;
use mcrs_minecraft_engine::bus::PacketPayload;
use mcrs_minecraft_engine::session::GameProfile;
use mcrs_minecraft_level::entity::physics::{Rotation, Transform};
use mcrs_minecraft_level::entity::player::Player;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundPlayerPosition;
use mcrs_minecraft_protocol::text::{IntoText, Text};
use mcrs_minecraft_protocol::{Look, PositionFlag, PositionFlags, VarInt};

#[derive(Resource, Clone, Copy)]
struct TeleportCommand {
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

/// CMD-TP-001.
fn build(r: &mut CommandRegistrar<'_>) -> Result<TeleportCommand, BuildError> {
    let open = PermissionCheck::AlwaysPass;
    let op = PermissionCheck::Require(CommandLevel::Gamemasters);
    let teleport = r.literal(CommandRegistrar::ROOT, "teleport", op)?;
    let location = r.argument(teleport, "location", Vec3Argument, open)?;
    r.executes(location.node())?;
    let destination = r.argument(teleport, "destination", EntityArgument::ONE_ENTITY, open)?;
    r.executes(destination.node())?;
    let targets = r.argument(teleport, "targets", EntityArgument::ENTITIES, open)?;
    let target_location = r.argument(targets.node(), "location", Vec3Argument, open)?;
    r.executes(target_location.node())?;
    let rotation = r.argument(target_location.node(), "rotation", RotationArgument, open)?;
    r.executes(rotation.node())?;
    let facing = r.literal(target_location.node(), "facing", open)?;
    let entity = r.literal(facing, "entity", open)?;
    let facing_entity = r.argument(entity, "facingEntity", EntityArgument::ONE_ENTITY, open)?;
    r.executes(facing_entity.node())?;
    let facing_anchor = r.argument(
        facing_entity.node(),
        "facingAnchor",
        EntityAnchorArgument,
        open,
    )?;
    r.executes(facing_anchor.node())?;
    let facing_location = r.argument(facing, "facingLocation", Vec3Argument, open)?;
    r.executes(facing_location.node())?;
    let target_destination = r.argument(
        targets.node(),
        "destination",
        EntityArgument::ONE_ENTITY,
        open,
    )?;
    r.executes(target_destination.node())?;
    let tp = r.literal(CommandRegistrar::ROOT, "tp", op)?;
    r.redirect(tp, teleport)?;
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

pub(super) fn register(app: &mut App) {
    app.add_command(build, run);
}

const SPAWNABLE_XZ: f64 = 30_000_000.0;
const SPAWNABLE_Y: f64 = 20_000_000.0;

fn spawnable(position: DVec3) -> bool {
    let block = position.floor();
    (-SPAWNABLE_XZ..SPAWNABLE_XZ).contains(&block.x)
        && (-SPAWNABLE_XZ..SPAWNABLE_XZ).contains(&block.z)
        && (-SPAWNABLE_Y..SPAWNABLE_Y).contains(&block.y)
}

enum Facing {
    Point(DVec3),
    Entity(PlayerCandidate, Anchor),
}

/// Where each victim goes (CMD-TP-004).
struct Destination {
    position: DVec3,
    /// `None` keeps the rotation of the victim.
    rotation: Option<(f32, f32)>,
    relative_position: [bool; 3],
    relative_rotation: [bool; 2],
    facing: Option<Facing>,
    to_entity: Option<PlayerCandidate>,
}

const PLAYER_EYE_HEIGHT: f64 = 1.62;

fn anchor_of(candidate: &PlayerCandidate, anchor: Anchor) -> DVec3 {
    match anchor {
        Anchor::Feet => candidate.transform.translation,
        Anchor::Eyes => candidate.transform.translation + DVec3::Y * PLAYER_EYE_HEIGHT,
    }
}

/// The yaw and the pitch that look from `from` at `to`.
fn look_at(from: DVec3, to: DVec3) -> (f32, f32) {
    let d = to - from;
    let horizontal = (d.x * d.x + d.z * d.z).sqrt();
    let pitch = -(d.y.atan2(horizontal).to_degrees()) as f32;
    let yaw = (d.z.atan2(d.x).to_degrees() - 90.0) as f32;
    (wrap_degrees(yaw), wrap_degrees(pitch))
}

fn one(found: Vec<&PlayerCandidate>) -> Result<&PlayerCandidate, CommandFailure> {
    match found.as_slice() {
        [one] => Ok(one),
        [] => Err(CommandFailure::translate(
            "argument.entity.notfound.entity",
            vec![],
        )),
        _ => Err(CommandFailure::translate("argument.entity.toomany", vec![])),
    }
}

fn coordinates(
    tree: &CommandTree,
    invocation: &Invocation,
    arg: Arg<Vec3Argument>,
    source: &Source<'_, DimLookup<'_>>,
) -> Result<Option<Coordinates>, CommandFailure> {
    invocation.get(tree, arg, source).transpose()
}

fn destination(
    command: &TeleportCommand,
    tree: &CommandTree,
    invocation: &Invocation,
    source: &Source<'_, DimLookup<'_>>,
    all: &[PlayerCandidate],
) -> Result<Destination, CommandFailure> {
    for arg in [command.destination, command.target_destination] {
        if invocation.has(arg) {
            let entity = one(select(tree, invocation, arg, source, all)?)?.clone();
            return Ok(Destination {
                position: entity.transform.translation,
                rotation: Some((
                    entity.transform.rotation.yaw(),
                    entity.transform.rotation.pitch(),
                )),
                relative_position: [false; 3],
                relative_rotation: [false; 2],
                facing: None,
                to_entity: Some(entity),
            });
        }
    }
    let location = coordinates(tree, invocation, command.location, source)?
        .or(coordinates(
            tree,
            invocation,
            command.target_location,
            source,
        )?)
        .ok_or_else(|| CommandFailure::translate("command.failed", vec![]))?;
    let rotation = invocation.get(tree, command.rotation, source).transpose()?;
    let facing = if invocation.has(command.facing_entity) {
        let entity = one(select(
            tree,
            invocation,
            command.facing_entity,
            source,
            all,
        )?)?
        .clone();
        let anchor = invocation
            .get(tree, command.facing_anchor, source)
            .transpose()?
            .unwrap_or(Anchor::Feet);
        Some(Facing::Entity(entity, anchor))
    } else {
        coordinates(tree, invocation, command.facing_location, source)?
            .map(|point| Facing::Point(point.position(source)))
    };
    Ok(Destination {
        position: location.position(source),
        rotation: rotation.map(|rotation| rotation.resolve(source)),
        relative_position: location.relative(),
        relative_rotation: rotation.map_or([true; 2], |r| [r.yaw.relative, r.pitch.relative]),
        facing,
        to_entity: None,
    })
}

/// The `player_position` of one move (CMD-TP-005, CMD-TP-006).
fn position_packet(
    teleport_id: i32,
    from: &Transform,
    to: &Destination,
    rotation: Rotation,
) -> ClientboundPlayerPosition {
    let mut flags = Vec::new();
    let mut position = to.position;
    for (axis, ((flag, velocity), relative)) in [
        (PositionFlag::X, PositionFlag::DeltaX),
        (PositionFlag::Y, PositionFlag::DeltaY),
        (PositionFlag::Z, PositionFlag::DeltaZ),
    ]
    .into_iter()
    .zip(to.relative_position)
    .enumerate()
    {
        if relative {
            flags.extend([flag, velocity]);
            position[axis] -= from.translation[axis];
        }
    }
    let mut look = Look {
        yaw: rotation.yaw(),
        pitch: rotation.pitch(),
    };
    if to.facing.is_none() {
        if to.relative_rotation[0] {
            flags.push(PositionFlag::YRot);
            look.yaw = wrap_degrees(rotation.yaw() - from.rotation.yaw());
        }
        if to.relative_rotation[1] {
            flags.push(PositionFlag::XRot);
            look.pitch = wrap_degrees(rotation.pitch() - from.rotation.pitch());
        }
    }
    ClientboundPlayerPosition {
        teleport_id: VarInt(teleport_id),
        position,
        velocity: DVec3::ZERO,
        look,
        flags: PositionFlags(flags),
    }
}

fn coordinate(value: f64) -> Text {
    format!("{value:.6}").into_text()
}

fn run(
    In(invocation): In<Invocation>,
    command: Res<TeleportCommand>,
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
    let lookup = context.lookup();
    let source = invocation.source(&lookup);
    let all = candidates(
        players
            .iter()
            .map(|(entity, profile, transform, mode, _, _)| (entity, profile, transform, mode)),
    );
    let victims: Vec<PlayerCandidate> = if invocation.has(command.targets) {
        let found = select(&context.tree, &invocation, command.targets, &source, &all)?;
        if found.is_empty() {
            return Err(CommandFailure::translate(
                "argument.entity.notfound.entity",
                vec![],
            ));
        }
        found.into_iter().cloned().collect()
    } else {
        all.iter()
            .filter(|candidate| candidate.entity == invocation.player)
            .cloned()
            .collect()
    };
    let to = destination(&command, &context.tree, &invocation, &source, &all)?;
    if !spawnable(to.position) {
        return Err(CommandFailure::translate(
            "commands.teleport.invalidPosition",
            vec![],
        ));
    }
    let mut moved: Vec<&PlayerCandidate> = Vec::new();
    for victim in &victims {
        let Ok((_, _, mut transform, _, mut state, &HostAnchor(host))) =
            players.get_mut(victim.entity)
        else {
            continue;
        };
        let from = *transform;
        let (yaw, pitch) = match &to.facing {
            Some(Facing::Point(point)) => look_at(to.position, *point),
            Some(Facing::Entity(entity, anchor)) => {
                look_at(to.position, anchor_of(entity, *anchor))
            }
            None => to
                .rotation
                .unwrap_or((from.rotation.yaw(), from.rotation.pitch())),
        };
        let rotation = Rotation::new(yaw, pitch);
        let moved_to = Transform {
            translation: to.position,
            rotation,
        };
        let teleport_id = state.sent(moved_to);
        *transform = moved_to;
        feedback.send(
            mcrs_minecraft_engine::bus::to(
                host,
                PacketPayload::PlayerPosition(position_packet(teleport_id, &from, &to, rotation)),
            )
            .critical(),
        );
        moved.push(victim);
    }
    let message = match (moved.as_slice(), &to.to_entity) {
        ([], _) => return Ok(0),
        ([one], Some(entity)) => Text::translate(
            "commands.teleport.success.entity.single",
            vec![name(one), name(entity)],
        ),
        (many, Some(entity)) => Text::translate(
            "commands.teleport.success.entity.multiple",
            vec![many.len().to_string().into_text(), name(entity)],
        ),
        ([one], None) => Text::translate(
            "commands.teleport.success.location.single",
            vec![
                name(one),
                coordinate(to.position.x),
                coordinate(to.position.y),
                coordinate(to.position.z),
            ],
        ),
        (many, None) => Text::translate(
            "commands.teleport.success.location.multiple",
            vec![
                many.len().to_string().into_text(),
                coordinate(to.position.x),
                coordinate(to.position.y),
                coordinate(to.position.z),
            ],
        ),
    };
    feedback.success(&invocation, message, true);
    Ok(i32::try_from(moved.len()).unwrap_or(i32::MAX))
}

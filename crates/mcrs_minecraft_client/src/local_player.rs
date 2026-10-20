use bevy::math::DVec3;
use bevy::prelude::*;
use bevy::window::{CursorOptions, PrimaryWindow};
use mcrs_minecraft_level::entity::physics::{
    OldTransform, Rotation, Transform as PhysicsTransform, Velocity,
};
use mcrs_minecraft_network::ConnectionState;
use mcrs_minecraft_network::client::{ClientConnection, PendingTeleports};
use mcrs_minecraft_protocol::packets::game::serverbound::{
    ServerboundAcceptTeleportation, ServerboundMovePlayerPos, ServerboundMovePlayerPosRot,
    ServerboundMovePlayerRot,
};
use mcrs_minecraft_protocol::{Look, MoveFlags, VarInt, WritePacket};

#[cfg(feature = "dev")]
use crate::dev::flight::ScriptedFlight;
use crate::input::{self, Input};
use crate::player::Player;

pub const TICKS_PER_SECOND: f64 = 20.0;

const SPRINT_SPEED_MULTIPLIER: f64 = 2.0;

#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Flying;

/// Blocks per tick.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct FlyingSpeed(pub f64);

impl Default for FlyingSpeed {
    fn default() -> Self {
        Self(0.5)
    }
}

#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Sprint {
    pub active: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MovePacket {
    PosRot,
    Pos,
    Rot,
}

/// What the server was last told, which is what this tick's position and look
/// are judged against.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct LastSentMovement {
    position: DVec3,
    rotation: Rotation,
}

impl LastSentMovement {
    fn tick(&mut self, position: DVec3, rotation: Rotation) -> Option<MovePacket> {
        let moved = position != self.position;
        let turned = rotation != self.rotation;
        self.position = position;
        self.rotation = rotation;
        match (moved, turned) {
            (true, true) => Some(MovePacket::PosRot),
            (true, false) => Some(MovePacket::Pos),
            (false, true) => Some(MovePacket::Rot),
            (false, false) => None,
        }
    }
}

/// The look a run asked for, kept through the teleports the server answers a join with.
#[derive(Resource, Clone, Copy)]
pub struct LookOverride {
    pub yaw: f32,
    pub pitch: f32,
}

/// The position a run asked for, kept through the teleports the server answers a join with.
#[derive(Resource, Clone, Copy)]
pub struct PositionOverride(pub DVec3);

/// One tick of local-player movement.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct LocalPlayerTick;

pub struct LocalPlayerPlugin;

impl Plugin for LocalPlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            (
                accept_teleports.before(LocalPlayerTick),
                (capture_old_transform, fly).chain().in_set(LocalPlayerTick),
                send_movement.after(LocalPlayerTick),
            ),
        );
    }
}

/// Moves to where the server says, then confirms with the position that move
/// landed on.
fn accept_teleports(
    player: Single<(&mut PhysicsTransform, &mut Velocity), With<Player>>,
    connection: Option<Single<(&mut ClientConnection, &mut PendingTeleports)>>,
    look: Option<Res<LookOverride>>,
    position: Option<Res<PositionOverride>>,
) {
    let Some(connection) = connection else { return };
    let (mut connection, mut pending) = connection.into_inner();
    if pending.0.is_empty() {
        return;
    }
    let (mut transform, mut velocity) = player.into_inner();
    for teleport in pending.0.drain(..) {
        transform.translation = position
            .as_deref()
            .map_or(teleport.position, |position| position.0);
        transform.rotation = match look.as_deref() {
            Some(look) => Rotation::new(look.yaw, look.pitch),
            None => Rotation::new(teleport.look.yaw, teleport.look.pitch),
        };
        velocity.0 = teleport.velocity;
        connection.write_packet(&ServerboundAcceptTeleportation {
            teleport_id: VarInt(teleport.teleport_id),
            position: transform.translation.into(),
            look: teleport.look,
        });
    }
}

/// Without it the server's view of the player never moves, so it never streams
/// the columns the player is flying towards.
fn send_movement(
    player: Single<(&PhysicsTransform, &mut LastSentMovement), With<Player>>,
    connection: Option<Single<(&mut ClientConnection, &ConnectionState)>>,
) {
    let Some(connection) = connection else { return };
    let (mut connection, state) = connection.into_inner();
    if *state != ConnectionState::Game {
        return;
    }

    let (transform, mut last_sent) = player.into_inner();
    // Flying never touches the ground and never collides, so both flags stay
    // clear until the client grows a collision step.
    let flags = MoveFlags::new();
    let Some(packet) = last_sent.tick(transform.translation, transform.rotation) else {
        return;
    };

    let position = transform.translation.into();
    let look = Look {
        yaw: transform.rotation.yaw(),
        pitch: transform.rotation.pitch(),
    };
    match packet {
        MovePacket::PosRot => connection.write_packet(&ServerboundMovePlayerPosRot {
            position,
            look,
            flags,
        }),
        MovePacket::Pos => connection.write_packet(&ServerboundMovePlayerPos { position, flags }),
        MovePacket::Rot => connection.write_packet(&ServerboundMovePlayerRot { look, flags }),
    }
}

fn capture_old_transform(player: Single<(&PhysicsTransform, &mut OldTransform), With<Player>>) {
    let (transform, mut old_transform) = player.into_inner();
    old_transform.0 = *transform;
}

fn fly(
    keys: Res<ButtonInput<KeyCode>>,
    cursor: Single<&CursorOptions, With<PrimaryWindow>>,
    #[cfg(feature = "dev")] scripted: Option<ResMut<ScriptedFlight>>,
    #[cfg(feature = "dev")] time: Res<Time>,
    player: Single<
        (&mut Sprint, &mut PhysicsTransform, &FlyingSpeed),
        (With<Player>, With<Flying>),
    >,
) {
    let (mut sprint, mut transform, flying_speed) = player.into_inner();
    #[cfg(feature = "dev")]
    let input = match scripted {
        Some(mut scripted) => scripted.input(&mut transform, time.elapsed_secs()),
        None => input::pressed(&keys, &cursor),
    };
    #[cfg(not(feature = "dev"))]
    let input = input::pressed(&keys, &cursor);
    let yaw = transform.rotation.yaw();
    transform.translation += tick(&mut sprint, *flying_speed, yaw, input);
}

/// The displacement of one tick: the pressed direction turned by the yaw, with
/// no inertia and no friction.
fn tick(sprint: &mut Sprint, flying_speed: FlyingSpeed, yaw: f32, input: Input) -> DVec3 {
    let move_vector = input::move_vector(input);
    sprint.active = input.sprint && move_vector.y > 0.0;
    let speed = if sprint.active {
        flying_speed.0 * SPRINT_SPEED_MULTIPLIER
    } else {
        flying_speed.0
    };
    let (sin, cos) = f64::from(yaw.to_radians()).sin_cos();
    let horizontal = DVec3::new(
        move_vector.x * cos - move_vector.y * sin,
        0.0,
        move_vector.y * cos + move_vector.x * sin,
    );
    let vertical = f64::from(i8::from(input.jump) - i8::from(input.shift));
    (horizontal + DVec3::Y * vertical) * speed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::tests::minecraft_look_direction;

    #[test]
    fn forward_moves_along_the_horizontal_look_direction() {
        let forward = Input {
            forward: true,
            ..Input::EMPTY
        };
        for yaw in [0.0, 90.0, -139.9493, 180.0] {
            let moved = tick(&mut Sprint::default(), FlyingSpeed(1.0), yaw, forward);
            let look = minecraft_look_direction(yaw, 0.0).as_dvec3();
            assert!(
                moved.abs_diff_eq(look, 1e-6),
                "yaw {yaw}: {moved:?} != {look:?}"
            );
        }
    }
}

use bevy::math::DVec3;
use bevy::prelude::*;
use mcrs_minecraft_engine_world::entity::physics::{
    OldTransform, Rotation, Transform as PhysicsTransform, Velocity,
};
use mcrs_minecraft_network::ConnectionState;
use mcrs_minecraft_network::client::ClientConnection;
use mcrs_minecraft_physics::{
    PlayerRotation, PlayerState, Relative, SineTable, Teleport, apply_teleport,
};
use mcrs_minecraft_protocol::{VarInt, WritePacket};
use mcrs_minecraft_protocol_game::MoveFlags;
use mcrs_minecraft_protocol_game::serverbound::{
    ServerboundAcceptTeleportation, ServerboundMovePlayerPos, ServerboundMovePlayerPosRot,
    ServerboundMovePlayerRot,
};

use crate::movement_input::MovementInput;
use crate::player::Player;
use crate::player_view::{PendingTeleports, PlayerViewPlugin, ServerTeleport};

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

/// The position a run asked for, in place of the first absolute teleport the server sends.
#[derive(Resource, Clone, Copy)]
pub struct PositionOverride(pub DVec3);

/// The sine table that rotates the velocity of a teleport.
#[derive(Resource, Clone, Debug, Default)]
pub struct ClientSine(pub SineTable);

/// One tick of local-player movement.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct LocalPlayerTick;

pub struct LocalPlayerPlugin;

impl Plugin for LocalPlayerPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<PlayerViewPlugin>() {
            app.add_plugins(PlayerViewPlugin);
        }
        app.init_resource::<MovementInput>()
            .init_resource::<ClientSine>()
            .add_systems(
                FixedUpdate,
                (
                    accept_teleports.before(LocalPlayerTick),
                    (capture_old_transform, fly).chain().in_set(LocalPlayerTick),
                    send_movement.after(LocalPlayerTick),
                ),
            );
    }
}

fn apply_server_teleport(
    sine: &SineTable,
    transform: &mut PhysicsTransform,
    velocity: &mut Velocity,
    teleport: &ServerTeleport,
) {
    let mut state = PlayerState::new(transform.translation);
    state.velocity = velocity.0;
    let mut rotation = PlayerRotation {
        yaw: transform.rotation.yaw(),
        pitch: transform.rotation.pitch(),
    };
    let packet = Teleport {
        position: teleport.position,
        velocity: teleport.velocity,
        yaw: teleport.look.yaw,
        pitch: teleport.look.pitch,
        relative: Relative::from_bits_truncate(teleport.flags.bits()),
    };
    apply_teleport(sine, &mut state, &mut rotation, &packet);
    transform.translation = state.position;
    transform.rotation = Rotation::new(rotation.yaw, rotation.pitch);
    velocity.0 = state.velocity;
}

/// Moves to where the server says, then confirms with the position that move
/// landed on.
fn accept_teleports(
    mut commands: Commands,
    player: Single<(&mut PhysicsTransform, &mut Velocity), With<Player>>,
    connection: Option<Single<(&mut ClientConnection, &mut PendingTeleports)>>,
    look: Option<Res<LookOverride>>,
    position: Option<Res<PositionOverride>>,
    sine: Res<ClientSine>,
) {
    let Some(connection) = connection else { return };
    let (mut connection, mut pending) = connection.into_inner();
    if pending.0.is_empty() {
        return;
    }
    let (mut transform, mut velocity) = player.into_inner();
    let mut position = position.map(|position| position.0);
    for teleport in pending.0.drain(..) {
        apply_server_teleport(&sine.0, &mut transform, &mut velocity, &teleport);
        if teleport.flags.is_empty() {
            if let Some(position) = position.take() {
                transform.translation = position;
                // The server answers the mismatch with its own destination,
                // which the client has to accept instead of overriding again.
                commands.remove_resource::<PositionOverride>();
            }
            if let Some(look) = look.as_deref() {
                transform.rotation = Rotation::new(look.yaw, look.pitch);
            }
        }
        connection.write_packet(&ServerboundAcceptTeleportation {
            teleport_id: VarInt(teleport.teleport_id),
            position: transform.translation.into(),
            look: transform.rotation.into(),
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
    let look = transform.rotation.into();
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
    input: Res<MovementInput>,
    player: Single<
        (&mut Sprint, &mut PhysicsTransform, &FlyingSpeed),
        (With<Player>, With<Flying>),
    >,
) {
    let (mut sprint, mut transform, flying_speed) = player.into_inner();
    let yaw = transform.rotation.yaw();
    transform.translation += tick(&mut sprint, *flying_speed, yaw, *input);
}

/// The displacement of one tick: the pressed direction turned by the yaw, with
/// no inertia and no friction.
fn tick(sprint: &mut Sprint, flying_speed: FlyingSpeed, yaw: f32, input: MovementInput) -> DVec3 {
    let move_vector = input.move_vector();
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
    use crate::player::look_direction;
    use mcrs_minecraft_core::Look;
    use mcrs_minecraft_protocol_game::PositionFlag;

    #[test]
    fn forward_moves_along_the_horizontal_look_direction() {
        let forward = MovementInput {
            forward: true,
            ..MovementInput::EMPTY
        };
        for yaw in [0.0, 90.0, -139.9493, 180.0] {
            let moved = tick(&mut Sprint::default(), FlyingSpeed(1.0), yaw, forward);
            let look = look_direction(yaw, 0.0).as_dvec3();
            assert!(
                moved.abs_diff_eq(look, 1e-6),
                "yaw {yaw}: {moved:?} != {look:?}"
            );
        }
    }

    fn teleport(flags: &[PositionFlag], look: Look, velocity: DVec3) -> ServerTeleport {
        ServerTeleport {
            teleport_id: 1,
            position: DVec3::new(1.0, 2.0, 3.0),
            velocity,
            look,
            flags: flags.iter().copied().collect(),
        }
    }

    fn landing(teleport: &ServerTeleport) -> (PhysicsTransform, Velocity) {
        let mut transform = PhysicsTransform {
            translation: DVec3::new(10.0, 64.0, -5.0),
            rotation: Rotation::new(45.0, 10.0),
        };
        let mut velocity = Velocity(DVec3::new(1.0, 0.0, -1.0));
        apply_server_teleport(&SineTable::new(), &mut transform, &mut velocity, teleport);
        (transform, velocity)
    }

    #[test]
    fn each_relative_flag_adds_to_its_own_component() {
        let look = Look {
            yaw: 90.0,
            pitch: -5.0,
        };
        let change = DVec3::new(0.5, 1.0, -0.5);
        let (transform, velocity) = landing(&teleport(&[], look, change));
        assert_eq!(transform.translation, DVec3::new(1.0, 2.0, 3.0));
        assert_eq!(
            (transform.rotation.yaw(), transform.rotation.pitch()),
            (90.0, -5.0)
        );
        assert_eq!(velocity.0, change);

        let cases = [
            (PositionFlag::X, DVec3::new(11.0, 2.0, 3.0)),
            (PositionFlag::Y, DVec3::new(1.0, 66.0, 3.0)),
            (PositionFlag::Z, DVec3::new(1.0, 2.0, -2.0)),
        ];
        for (flag, expected) in cases {
            let (transform, _) = landing(&teleport(&[flag], look, change));
            assert_eq!(transform.translation, expected, "{flag:?}");
        }
        let (transform, _) = landing(&teleport(&[PositionFlag::YRot], look, change));
        assert_eq!(
            (transform.rotation.yaw(), transform.rotation.pitch()),
            (135.0, -5.0)
        );
        let (transform, _) = landing(&teleport(&[PositionFlag::XRot], look, change));
        assert_eq!(
            (transform.rotation.yaw(), transform.rotation.pitch()),
            (90.0, 5.0)
        );
        let cases = [
            (PositionFlag::DeltaX, DVec3::new(1.5, 1.0, -0.5)),
            (PositionFlag::DeltaY, DVec3::new(0.5, 1.0, -0.5)),
            (PositionFlag::DeltaZ, DVec3::new(0.5, 1.0, -1.5)),
        ];
        for (flag, expected) in cases {
            let (_, velocity) = landing(&teleport(&[flag], look, change));
            assert_eq!(velocity.0, expected, "{flag:?}");
        }
    }

    #[test]
    fn rotate_delta_turns_the_velocity_with_the_yaw_change() {
        let flags = [
            PositionFlag::YRot,
            PositionFlag::XRot,
            PositionFlag::RotateDelta,
            PositionFlag::DeltaX,
            PositionFlag::DeltaY,
            PositionFlag::DeltaZ,
        ];
        let look = Look {
            yaw: 90.0,
            pitch: 0.0,
        };
        let (_, velocity) = landing(&teleport(&flags, look, DVec3::ZERO));
        assert!(
            velocity.0.abs_diff_eq(DVec3::new(1.0, 0.0, 1.0), 1e-6),
            "{:?}",
            velocity.0
        );
    }
}

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

/// The position a run asked for, kept through the teleports the server answers a join with.
#[derive(Resource, Clone, Copy)]
pub struct PositionOverride(pub DVec3);

/// The client's sine table for trigonometry calculations, such as relative teleport velocity rotations.
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

/// Computes the resulting transform and velocity from applying a server teleport (absolute or relative)
/// to the player's current transform and velocity.
#[must_use]
pub fn calculate_teleport(
    sine: &SineTable,
    current_transform: PhysicsTransform,
    current_velocity: Velocity,
    teleport: &ServerTeleport,
) -> (PhysicsTransform, Velocity) {
    let relative = Relative::from_bits_truncate(teleport.flags.bits());
    let teleport_spec = Teleport {
        position: teleport.position,
        velocity: teleport.velocity,
        yaw: teleport.look.yaw,
        pitch: teleport.look.pitch,
        relative,
    };
    let mut state = PlayerState::new(current_transform.translation);
    state.velocity = current_velocity.0;
    let mut player_rotation = PlayerRotation {
        yaw: current_transform.rotation.yaw(),
        pitch: current_transform.rotation.pitch(),
    };
    apply_teleport(sine, &mut state, &mut player_rotation, &teleport_spec);
    (
        PhysicsTransform {
            translation: state.position,
            rotation: Rotation::new(player_rotation.yaw, player_rotation.pitch),
        },
        Velocity(state.velocity),
    )
}

/// Moves to where the server says, then confirms with the position that move
/// landed on.
fn accept_teleports(
    player: Single<(&mut PhysicsTransform, &mut Velocity), With<Player>>,
    connection: Option<Single<(&mut ClientConnection, &mut PendingTeleports)>>,
    look: Option<Res<LookOverride>>,
    position: Option<Res<PositionOverride>>,
    sine: Option<Res<ClientSine>>,
) {
    let Some(connection) = connection else { return };
    let (mut connection, mut pending) = connection.into_inner();
    if pending.0.is_empty() {
        return;
    }
    let (mut transform, mut velocity) = player.into_inner();
    let fallback_sine;
    let sine_table = match sine.as_deref() {
        Some(sine) => &sine.0,
        None => {
            fallback_sine = SineTable::new();
            &fallback_sine
        }
    };

    for teleport in pending.0.drain(..) {
        let (new_transform, new_velocity) =
            calculate_teleport(sine_table, *transform, *velocity, &teleport);

        // Position and look overrides apply to initial absolute join teleports;
        // relative teleports apply their delta to the player's current state.
        transform.translation = match position.as_deref() {
            Some(pos_override) if teleport.flags.is_empty() => pos_override.0,
            _ => new_transform.translation,
        };
        transform.rotation = match look.as_deref() {
            Some(look_override) if teleport.flags.is_empty() => {
                Rotation::new(look_override.yaw, look_override.pitch)
            }
            _ => new_transform.rotation,
        };
        velocity.0 = new_velocity.0;

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
    use mcrs_minecraft_protocol_game::{PositionFlag, PositionFlags};

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

    #[test]
    fn relative_teleport_adds_to_position_rotation_and_velocity() {
        let sine = SineTable::new();
        let initial_transform = PhysicsTransform {
            translation: DVec3::new(10.0, 64.0, -5.0),
            rotation: Rotation::new(45.0, 10.0),
        };
        let initial_velocity = Velocity(DVec3::new(1.0, 0.0, -1.0));

        // Absolute teleport
        let abs_teleport = ServerTeleport {
            teleport_id: 1,
            position: DVec3::new(20.0, 70.0, 30.0),
            velocity: DVec3::new(0.0, 0.5, 0.0),
            look: Look {
                yaw: 90.0,
                pitch: 0.0,
            },
            flags: PositionFlags::default(),
        };
        let (trans, vel) =
            calculate_teleport(&sine, initial_transform, initial_velocity, &abs_teleport);
        assert_eq!(trans.translation, DVec3::new(20.0, 70.0, 30.0));
        assert_eq!(trans.rotation.yaw(), 90.0);
        assert_eq!(trans.rotation.pitch(), 0.0);
        assert_eq!(vel.0, DVec3::new(0.0, 0.5, 0.0));

        // Relative position: /tp ~ ~10 ~
        let rel_pos_teleport = ServerTeleport {
            teleport_id: 2,
            position: DVec3::new(0.0, 10.0, 0.0),
            velocity: DVec3::ZERO,
            look: Look {
                yaw: 0.0,
                pitch: 0.0,
            },
            flags: [PositionFlag::X, PositionFlag::Y, PositionFlag::Z]
                .into_iter()
                .collect(),
        };
        let (trans, vel) =
            calculate_teleport(&sine, initial_transform, initial_velocity, &rel_pos_teleport);
        assert_eq!(trans.translation, DVec3::new(10.0, 74.0, -5.0));
        assert_eq!(vel.0, DVec3::ZERO);

        // Relative rotation: yaw + 90, pitch - 5
        let rel_rot_teleport = ServerTeleport {
            teleport_id: 3,
            position: DVec3::ZERO,
            velocity: DVec3::ZERO,
            look: Look {
                yaw: 90.0,
                pitch: -5.0,
            },
            flags: [PositionFlag::YRot, PositionFlag::XRot]
                .into_iter()
                .collect(),
        };
        let (trans, _) =
            calculate_teleport(&sine, initial_transform, initial_velocity, &rel_rot_teleport);
        assert_eq!(trans.rotation.yaw(), 135.0);
        assert_eq!(trans.rotation.pitch(), 5.0);

        // Relative velocity: delta x, y, z
        let rel_vel_teleport = ServerTeleport {
            teleport_id: 4,
            position: DVec3::ZERO,
            velocity: DVec3::new(0.5, 1.0, -0.5),
            look: Look {
                yaw: 0.0,
                pitch: 0.0,
            },
            flags: [PositionFlag::DeltaX, PositionFlag::DeltaY, PositionFlag::DeltaZ]
                .into_iter()
                .collect(),
        };
        let (_, vel) =
            calculate_teleport(&sine, initial_transform, initial_velocity, &rel_vel_teleport);
        assert_eq!(vel.0, DVec3::new(1.5, 1.0, -1.5));
    }
}

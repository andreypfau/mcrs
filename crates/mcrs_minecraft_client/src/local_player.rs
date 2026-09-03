use bevy::math::{DVec2, DVec3};
use bevy::prelude::*;
use bevy::window::{CursorOptions, PrimaryWindow};
use mcrs_minecraft_network::ConnectionState;
use mcrs_minecraft_network::client::{ClientConnection, PendingTeleports};
use mcrs_minecraft_protocol::packets::game::serverbound::{
    ServerboundAcceptTeleportation, ServerboundMovePlayerPos, ServerboundMovePlayerPosRot,
    ServerboundMovePlayerRot, ServerboundMovePlayerStatusOnly,
};
use mcrs_minecraft_protocol::{Look, MoveFlags, VarInt, WritePacket};
use mcrs_minecraft_world::entity::movement;
use mcrs_minecraft_world::entity::player::{Flying, FlyingSpeed, Input};
use mcrs_voxel_world::entity::physics::{
    OldTransform, Rotation, Transform as PhysicsTransform, Velocity,
};

use crate::input;
use crate::options::SPRINT_WINDOW_TICKS;
use crate::player::Player;

pub const TICKS_PER_SECOND: f64 = 20.0;

const VERTICAL_IMPULSE_SCALE: f64 = 3.0;

#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Sprint {
    pub active: bool,
    window: u8,
    moving_forward: bool,
    shifting: bool,
}

impl Sprint {
    /// The sprint trigger of `LocalPlayer.aiStep`. The retained booleans are
    /// last tick's, which is what the machine compares this tick's against.
    fn tick(&mut self, input: Input, move_vector: DVec2) {
        if self.window > 0 {
            self.window -= 1;
        }

        let moving_forward = move_vector.y > 1e-5;
        let was_moving_forward = std::mem::replace(&mut self.moving_forward, moving_forward);
        let was_shifting = std::mem::replace(&mut self.shifting, input.shift);

        if was_shifting || input.backward {
            self.window = 0;
        }
        if !self.active && moving_forward {
            if !was_moving_forward {
                if self.window > 0 {
                    self.active = true;
                } else {
                    self.window = SPRINT_WINDOW_TICKS;
                }
            }
            if input.sprint {
                self.active = true;
            }
        }
        if self.active && !moving_forward {
            self.active = false;
        }
    }
}

/// The squared distance `LocalPlayer.sendPosition` calls a move.
const MOVE_EPSILON_SQUARED: f64 = 2.0e-4 * 2.0e-4;

/// A stationary player still reports its position this often.
const POSITION_REMINDER_TICKS: u8 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MovePacket {
    PosRot,
    Pos,
    Rot,
    StatusOnly,
}

/// What the server was last told, which is what this tick's position and look
/// are judged against. Not derivable from the transform alone.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct LastSentMovement {
    position: DVec3,
    rotation: Rotation,
    flags: u8,
    reminder: u8,
}

impl LastSentMovement {
    fn tick(
        &mut self,
        position: DVec3,
        rotation: Rotation,
        flags: MoveFlags,
    ) -> Option<MovePacket> {
        self.reminder += 1;
        let moved = position.distance_squared(self.position) > MOVE_EPSILON_SQUARED
            || self.reminder >= POSITION_REMINDER_TICKS;
        let turned = rotation != self.rotation;
        let flags = flags.into_bits();

        let packet = match (moved, turned) {
            (true, true) => Some(MovePacket::PosRot),
            (true, false) => Some(MovePacket::Pos),
            (false, true) => Some(MovePacket::Rot),
            (false, false) if flags != self.flags => Some(MovePacket::StatusOnly),
            (false, false) => None,
        };

        if moved {
            self.position = position;
            self.reminder = 0;
        }
        if turned {
            self.rotation = rotation;
        }
        self.flags = flags;
        packet
    }
}

#[derive(Resource)]
pub struct ScriptedFlight;

/// One tick of local-player movement, from the sprint machine through the
/// shared travel step.
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

/// `ClientPacketListener.handleMovePlayer`: move to where the server says, then
/// confirm with the position that move landed on.
fn accept_teleports(
    player: Single<(&mut PhysicsTransform, &mut Velocity), With<Player>>,
    connection: Option<Single<(&mut ClientConnection, &mut PendingTeleports)>>,
) {
    let Some(connection) = connection else { return };
    let (mut connection, mut pending) = connection.into_inner();
    if pending.0.is_empty() {
        return;
    }
    let (mut transform, mut velocity) = player.into_inner();
    for teleport in pending.0.drain(..) {
        transform.translation = teleport.position;
        transform.rotation = Rotation::new(teleport.look.yaw, teleport.look.pitch);
        velocity.0 = teleport.velocity;
        connection.write_packet(&ServerboundAcceptTeleportation {
            teleport_id: VarInt(teleport.teleport_id),
            position: transform.translation.into(),
            look: teleport.look,
        });
    }
}

/// `LocalPlayer.sendPosition`. Without it the server's view of the player never
/// moves, so it never streams the columns the player is flying towards.
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
    let Some(packet) = last_sent.tick(transform.translation, transform.rotation, flags) else {
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
        MovePacket::StatusOnly => {
            connection.write_packet(&ServerboundMovePlayerStatusOnly { flags })
        }
    }
}

/// `Entity.commonTick` records where the entity was before anything moves it.
fn capture_old_transform(player: Single<(&PhysicsTransform, &mut OldTransform), With<Player>>) {
    let (transform, mut old_transform) = player.into_inner();
    old_transform.0 = *transform;
}

#[allow(clippy::type_complexity)]
fn fly(
    keys: Res<ButtonInput<KeyCode>>,
    cursor: Single<&CursorOptions, With<PrimaryWindow>>,
    scripted: Option<Res<ScriptedFlight>>,
    player: Single<
        (
            &mut Sprint,
            &mut Velocity,
            &mut PhysicsTransform,
            &FlyingSpeed,
        ),
        (With<Player>, With<Flying>),
    >,
) {
    let (mut sprint, mut velocity, mut transform, flying_speed) = player.into_inner();
    let input = if scripted.is_some() {
        Input {
            forward: true,
            sprint: true,
            ..Input::EMPTY
        }
    } else {
        input::pressed(&keys, &cursor)
    };
    let yaw = transform.rotation.yaw();
    tick(
        &mut sprint,
        &mut velocity.0,
        &mut transform.translation,
        *flying_speed,
        yaw,
        input,
    );
}

fn tick(
    sprint: &mut Sprint,
    velocity: &mut DVec3,
    position: &mut DVec3,
    flying_speed: FlyingSpeed,
    yaw: f32,
    input: Input,
) {
    let move_vector = input::move_vector(input);
    sprint.tick(input, move_vector);

    let vertical = f64::from(i8::from(input.jump) - i8::from(input.shift));
    if vertical != 0.0 {
        velocity.y += vertical * flying_speed.0 * VERTICAL_IMPULSE_SCALE;
    }

    movement::snap_tiny_movement_of_player(velocity);
    *position += movement::travel_flying(
        velocity,
        input::modify_input(move_vector),
        flying_speed.with_sprint(sprint.active),
        yaw,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::tests::minecraft_look_direction;

    struct Local {
        sprint: Sprint,
        velocity: DVec3,
        position: DVec3,
    }

    impl Local {
        fn new() -> Self {
            Self {
                sprint: Sprint::default(),
                velocity: DVec3::ZERO,
                position: DVec3::ZERO,
            }
        }

        fn tick(&mut self, yaw: f32, input: Input) {
            tick(
                &mut self.sprint,
                &mut self.velocity,
                &mut self.position,
                FlyingSpeed::default(),
                yaw,
                input,
            );
        }

        fn step(&mut self, yaw: f32, input: Input) -> DVec3 {
            let before = self.position;
            self.tick(yaw, input);
            self.position - before
        }
    }

    fn forward() -> Input {
        Input {
            forward: true,
            ..Input::EMPTY
        }
    }

    fn at_terminal(input: Input) -> (Local, DVec3) {
        let mut local = Local::new();
        let mut displacement = DVec3::ZERO;
        for _ in 0..500 {
            displacement = local.step(0.0, input);
        }
        (local, displacement)
    }

    #[test]
    fn the_first_forward_tick_from_rest_moves_one_acceleration() {
        let displacement = Local::new().step(0.0, forward());
        assert!((displacement.z - 0.049).abs() < 1e-12, "{displacement:?}");
    }

    #[test]
    fn forward_settles_at_the_vanilla_terminal_speed() {
        let (_, displacement) = at_terminal(forward());
        assert!(
            (displacement.z - 0.5444444444444444).abs() < 1e-9,
            "{displacement:?}"
        );
    }

    #[test]
    fn sprinting_forward_settles_at_twice_the_terminal_speed() {
        let input = Input {
            sprint: true,
            ..forward()
        };
        let (local, displacement) = at_terminal(input);
        assert!(local.sprint.active);
        assert!(
            (displacement.z - 1.0888888888888888).abs() < 1e-9,
            "{displacement:?}"
        );
    }

    #[test]
    fn releasing_forward_at_terminal_glides_to_a_stop() {
        let (mut local, _) = at_terminal(forward());
        local.position = DVec3::ZERO;
        let (mut ticks, mut glided) = (0, 0.0);
        loop {
            let displacement = local.step(0.0, Input::EMPTY);
            ticks += 1;
            glided += displacement.length();
            if displacement.x == 0.0 && displacement.z == 0.0 {
                break;
            }
        }
        assert_eq!(ticks, 56);
        assert!((glided - 5.474175247121138).abs() < 1e-9, "{glided}");
    }

    #[test]
    fn one_jump_tick_decays_to_a_standstill() {
        let mut local = Local::new();
        let expected = [
            0.15, 0.09, 0.054, 0.0324, 0.01944, 0.011664, 0.0069984, 0.00419904, 0.0, 0.0, 0.0,
        ];
        for (tick, expected) in expected.into_iter().enumerate() {
            let input = if tick == 0 {
                Input {
                    jump: true,
                    ..Input::EMPTY
                }
            } else {
                Input::EMPTY
            };
            let displacement = local.step(0.0, input);
            assert!(
                (displacement.y - expected).abs() < 1e-12,
                "tick {tick}: {} != {expected}",
                displacement.y
            );
        }
        assert!(
            (local.position.y - 0.36870144).abs() < 1e-12,
            "{}",
            local.position.y
        );
    }

    #[test]
    fn jump_and_shift_together_cancel() {
        let mut local = Local::new();
        let input = Input {
            jump: true,
            shift: true,
            ..Input::EMPTY
        };
        for _ in 0..10 {
            assert_eq!(local.step(0.0, input).y, 0.0);
        }
    }

    /// Vanilla shapes keyboard input out to the unit *square*, so a diagonal is
    /// clamped to length 1 where a cardinal direction keeps the 0.98 scale.
    #[test]
    fn a_diagonal_runs_one_over_zero_point_nine_eight_faster_than_forward() {
        let distance = |input| {
            let mut local = Local::new();
            for _ in 0..100 {
                local.tick(0.0, input);
            }
            local.position.length()
        };
        let straight = distance(forward());
        let diagonal = distance(Input {
            left: true,
            ..forward()
        });
        assert!(
            (diagonal / straight - 1.0 / 0.98).abs() < 1e-9,
            "{straight} vs {diagonal}"
        );
    }

    #[test]
    fn forward_moves_along_the_horizontal_look_direction() {
        for yaw in [0.0, 37.5, 90.0, -139.9493, 180.0, -90.0] {
            let mut local = Local::new();
            for _ in 0..50 {
                local.tick(yaw, forward());
            }
            let look = minecraft_look_direction(yaw, 0.0);
            let moved = local.position.as_vec3().normalize();
            assert!(
                moved.abs_diff_eq(look, 1e-6),
                "yaw {yaw}: {moved:?} != {look:?}"
            );
        }
    }

    fn on_ground(on_ground: bool) -> MoveFlags {
        MoveFlags::new().with_on_ground(on_ground)
    }

    #[test]
    fn flying_forward_reports_every_tick_and_only_turns_when_the_look_moves() {
        let mut local = Local::new();
        let mut last_sent = LastSentMovement::default();
        let mut sent = Vec::new();
        for tick in 0..5 {
            local.tick(0.0, forward());
            let rotation = Rotation::new(if tick >= 3 { 90.0 } else { 0.0 }, 0.0);
            sent.push(last_sent.tick(local.position, rotation, on_ground(false)));
        }
        assert_eq!(
            sent,
            [
                Some(MovePacket::Pos),
                Some(MovePacket::Pos),
                Some(MovePacket::Pos),
                Some(MovePacket::PosRot),
                Some(MovePacket::Pos),
            ]
        );
        assert_eq!(last_sent.position, local.position);
    }

    #[test]
    fn a_stationary_player_reports_once_every_twenty_ticks() {
        let mut last_sent = LastSentMovement::default();
        let resting = DVec3::new(8.5, 80.0, -3.5);
        let facing = Rotation::new(45.0, 10.0);
        assert_eq!(
            last_sent.tick(resting, facing, on_ground(true)),
            Some(MovePacket::PosRot)
        );

        let mut reminders = Vec::new();
        for tick in 1..=60 {
            if last_sent.tick(resting, facing, on_ground(true)).is_some() {
                reminders.push(tick);
            }
        }
        assert_eq!(reminders, [20, 40, 60]);
    }

    #[test]
    fn looking_around_in_place_sends_rotation_only() {
        let mut last_sent = LastSentMovement::default();
        let resting = DVec3::ZERO;
        last_sent.tick(resting, Rotation::ZERO, on_ground(true));
        assert_eq!(
            last_sent.tick(resting, Rotation::new(1.0, 0.0), on_ground(true)),
            Some(MovePacket::Rot)
        );
    }

    #[test]
    fn a_flag_change_alone_sends_status_only() {
        let mut last_sent = LastSentMovement::default();
        let resting = DVec3::ZERO;
        last_sent.tick(resting, Rotation::ZERO, on_ground(true));
        assert_eq!(
            last_sent.tick(resting, Rotation::ZERO, on_ground(false)),
            Some(MovePacket::StatusOnly)
        );
        assert_eq!(
            last_sent.tick(resting, Rotation::ZERO, on_ground(false)),
            None
        );
    }

    /// A drift under the epsilon is not a move, so the reminder keeps counting
    /// and the position it eventually reports is the drifted one.
    #[test]
    fn a_sub_epsilon_drift_is_not_a_move() {
        let mut last_sent = LastSentMovement::default();
        let mut position = DVec3::ZERO;
        last_sent.tick(position, Rotation::ZERO, on_ground(true));
        for _ in 0..18 {
            position.x += 1.0e-5;
            assert_eq!(
                last_sent.tick(position, Rotation::ZERO, on_ground(true)),
                None
            );
        }
        position.x += 1.0e-5;
        assert_eq!(
            last_sent.tick(position, Rotation::ZERO, on_ground(true)),
            Some(MovePacket::Pos)
        );
        assert_eq!(last_sent.position, position);
    }

    #[test]
    fn double_tapping_forward_sprints_only_inside_the_window() {
        let tap_on_tick = |tick: u32| {
            let mut local = Local::new();
            for current in 1..=tick {
                let input = if current == 1 || current == tick {
                    forward()
                } else {
                    Input::EMPTY
                };
                local.tick(0.0, input);
            }
            local.sprint.active
        };
        assert!(tap_on_tick(7));
        assert!(!tap_on_tick(8));
    }

    #[test]
    fn releasing_the_sprint_key_does_not_stop_sprinting() {
        let mut local = Local::new();
        local.tick(
            0.0,
            Input {
                sprint: true,
                ..forward()
            },
        );
        assert!(local.sprint.active);
        for _ in 0..20 {
            local.tick(0.0, forward());
            assert!(local.sprint.active);
        }
        local.tick(0.0, Input::EMPTY);
        assert!(!local.sprint.active);
    }
}

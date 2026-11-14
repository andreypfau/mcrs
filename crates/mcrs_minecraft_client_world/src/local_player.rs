use bevy::math::DVec3;
use bevy::prelude::*;
use mcrs_minecraft_engine_world::entity::physics::{
    OldTransform, Rotation, Transform as PhysicsTransform, Velocity,
};
use mcrs_minecraft_entity::attribute_keys::Attribute;
use mcrs_minecraft_entity::{InstanceModifier, SPRINT_MODIFIER};
use mcrs_minecraft_network::ConnectionState;
use mcrs_minecraft_network::client::ClientConnection;
use mcrs_minecraft_physics::{Orientation, PlayerState, Relative, Teleport, apply_teleport};
use mcrs_minecraft_protocol::{VarInt, WritePacket};
use mcrs_minecraft_protocol_game::MoveFlags;
use mcrs_minecraft_protocol_game::serverbound::{
    ServerboundAcceptTeleportation, ServerboundClientTickEnd, ServerboundMovePlayerPos,
    ServerboundMovePlayerPosRot, ServerboundMovePlayerRot,
};

use crate::attribute_instances::{self, AttributeInstances};
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

#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
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
/// are judged against. Empty until the first teleport is accepted: a vanilla
/// server disconnects a client that moves before the server has placed it.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct LastSentMovement(Option<SentMovement>);

#[derive(Debug, Clone, Copy, PartialEq)]
struct SentMovement {
    position: DVec3,
    rotation: Rotation,
}

impl LastSentMovement {
    fn place(&mut self, position: DVec3, rotation: Rotation) {
        self.0 = Some(SentMovement { position, rotation });
    }

    fn tick(&mut self, position: DVec3, rotation: Rotation) -> Option<MovePacket> {
        let last = self.0.as_mut()?;
        let moved = position != last.position;
        let turned = rotation != last.rotation;
        *last = SentMovement { position, rotation };
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
            .add_observer(attribute_instances::on_login)
            .add_observer(attribute_instances::on_respawn)
            .add_observer(attribute_instances::on_update_attributes)
            .add_systems(
                FixedUpdate,
                (
                    accept_teleports.before(LocalPlayerTick),
                    (capture_old_transform, fly, give_sprint_modifier)
                        .chain()
                        .in_set(LocalPlayerTick),
                    send_movement.after(LocalPlayerTick),
                ),
            );
    }
}

fn apply_server_teleport(
    transform: &mut PhysicsTransform,
    velocity: &mut Velocity,
    teleport: &ServerTeleport,
) {
    let mut state = PlayerState::new(transform.translation);
    state.body.velocity = velocity.0;
    let mut rotation = Orientation::facing(transform.rotation.yaw(), transform.rotation.pitch());
    let packet = Teleport {
        position: teleport.position,
        velocity: teleport.velocity,
        yaw: teleport.look.yaw,
        pitch: teleport.look.pitch,
        relative: Relative::from_bits_truncate(teleport.flags.bits()),
    };
    apply_teleport(&mut state, &mut rotation, &packet);
    transform.translation = state.body.position;
    transform.rotation = Rotation::new(rotation.yaw, rotation.pitch);
    velocity.0 = state.body.velocity;
}

/// Moves to where the server says, then confirms with the position that move
/// landed on.
fn accept_teleports(
    mut commands: Commands,
    player: Single<(&mut PhysicsTransform, &mut Velocity, &mut LastSentMovement), With<Player>>,
    connection: Option<Single<(&mut ClientConnection, &mut PendingTeleports)>>,
    look: Option<Res<LookOverride>>,
    position: Option<Res<PositionOverride>>,
) {
    let Some(connection) = connection else { return };
    let (mut connection, mut pending) = connection.into_inner();
    if pending.0.is_empty() {
        return;
    }
    let (mut transform, mut velocity, mut last_sent) = player.into_inner();
    let mut position = position.map(|position| position.0);
    for teleport in pending.0.drain(..) {
        apply_server_teleport(&mut transform, &mut velocity, &teleport);
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
        last_sent.place(transform.translation, transform.rotation);
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
    if let Some(packet) = last_sent.tick(transform.translation, transform.rotation) {
        let position = transform.translation.into();
        let look = transform.rotation.into();
        match packet {
            MovePacket::PosRot => connection.write_packet(&ServerboundMovePlayerPosRot {
                position,
                look,
                flags,
            }),
            MovePacket::Pos => {
                connection.write_packet(&ServerboundMovePlayerPos { position, flags })
            }
            MovePacket::Rot => connection.write_packet(&ServerboundMovePlayerRot { look, flags }),
        }
    }
    connection.write_packet(&ServerboundClientTickEnd);
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
    let active = input.sprint && input.move_vector().y > 0.0;
    sprint.set_if_neq(Sprint { active });
    let yaw = transform.rotation.yaw();
    transform.translation += tick(*sprint, *flying_speed, yaw, *input);
}

/// Adds or removes the sprint modifier only when the sprint state changes (ATR-SRC-002), so a
/// `movement_speed` entry that drops it keeps it dropped until the next change (ATR-CLI-002).
fn give_sprint_modifier(
    mut players: Query<(&Sprint, &mut AttributeInstances), (With<Player>, Changed<Sprint>)>,
) {
    for (sprint, mut instances) in &mut players {
        if sprint.active {
            instances.put_modifier(
                Attribute::MovementSpeed,
                InstanceModifier::transient(&SPRINT_MODIFIER),
            );
        } else {
            instances.remove_modifier(Attribute::MovementSpeed, SPRINT_MODIFIER.id);
        }
    }
}

/// The displacement of one tick: the pressed direction turned by the yaw, with
/// no inertia and no friction.
fn tick(sprint: Sprint, flying_speed: FlyingSpeed, yaw: f32, input: MovementInput) -> DVec3 {
    let move_vector = input.move_vector();
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
    use crate::player::{look_direction, spawn_player};
    use mcrs_minecraft_core::Look;
    use mcrs_minecraft_dimension::dimension_keys::OVERWORLD;
    use mcrs_minecraft_entity::AttributeBases;
    use mcrs_minecraft_entity::entity_type_keys::EntityType;
    use mcrs_minecraft_game_mode::GameMode;
    use mcrs_minecraft_network::event::Received;
    use mcrs_minecraft_protocol::Encode;
    use mcrs_minecraft_protocol::packets::table::Row;
    use mcrs_minecraft_protocol_game::PositionFlag;
    use mcrs_minecraft_protocol_game::clientbound::{
        AttributeSnapshot, ClientboundLogin, ClientboundRespawn,
    };
    use mcrs_minecraft_protocol_game::player::PlayerSpawnInfo;
    use mcrs_minecraft_resource::Id;

    #[test]
    fn no_move_is_sent_before_the_first_accepted_teleport() {
        let mut last_sent = LastSentMovement::default();
        let rotation = Rotation::new(0.0, 0.0);
        assert!(
            last_sent
                .tick(DVec3::new(0.5, 80.0, 0.5), rotation)
                .is_none()
        );

        last_sent.place(DVec3::new(0.5, -60.0, 5.5), rotation);
        assert!(
            last_sent
                .tick(DVec3::new(0.5, -60.0, 5.5), rotation)
                .is_none()
        );
        assert!(matches!(
            last_sent.tick(DVec3::new(0.5, -60.0, 6.5), rotation),
            Some(MovePacket::Pos)
        ));
    }

    #[test]
    fn the_sprint_state_puts_and_removes_the_sprint_modifier() {
        let base = 0.1_f32 as f64;
        let sprinting = base * (1.0 + 0.3_f32 as f64);
        let mut app = App::new();
        app.add_systems(Update, give_sprint_modifier);
        let bases = AttributeBases::of(EntityType::Player).expect("a player has attributes");
        let player = app
            .world_mut()
            .spawn((Player, Sprint::default(), AttributeInstances::new(bases)))
            .id();
        let speed = |app: &App| {
            app.world()
                .get::<AttributeInstances>(player)
                .and_then(|instances| instances.value(Attribute::MovementSpeed))
                .map(f64::to_bits)
        };
        let set_sprint = |app: &mut App, active: bool| {
            app.world_mut()
                .get_mut::<Sprint>(player)
                .expect("the player sprints")
                .active = active;
            app.update();
        };

        set_sprint(&mut app, true);
        assert_eq!(speed(&app), Some(sprinting.to_bits()));

        app.world_mut()
            .get_mut::<AttributeInstances>(player)
            .expect("the player has instances")
            .receive(&AttributeSnapshot {
                attribute: Attribute::MovementSpeed,
                base,
                modifiers: Vec::new(),
            });
        app.update();
        assert_eq!(
            speed(&app),
            Some(base.to_bits()),
            "an entry drops the modifier"
        );

        set_sprint(&mut app, false);
        assert_eq!(speed(&app), Some(base.to_bits()));
    }

    #[test]
    fn a_held_sprint_key_sprints_again_after_a_respawn_and_a_login() {
        let sprinting = (0.1_f32 as f64 * (1.0 + 0.3_f32 as f64)).to_bits();
        let mut app = App::new();
        app.add_plugins(LocalPlayerPlugin);
        app.insert_resource(MovementInput {
            forward: true,
            sprint: true,
            ..MovementInput::EMPTY
        });
        let player = spawn_player(app.world_mut(), DVec3::ZERO, 0.0, 0.0);
        let connection = app.world_mut().spawn_empty().id();
        let speed = |app: &App| {
            app.world()
                .get::<AttributeInstances>(player)
                .and_then(|instances| instances.value(Attribute::MovementSpeed))
                .map(f64::to_bits)
        };
        let login = login();
        let respawn = ClientboundRespawn {
            player_spawn_info: spawn_info(),
            data_to_keep: 0,
        };

        trigger(&mut app, connection, &login);
        app.world_mut().run_schedule(FixedUpdate);
        assert_eq!(speed(&app), Some(sprinting));

        trigger(&mut app, connection, &respawn);
        app.world_mut().run_schedule(FixedUpdate);
        assert_eq!(speed(&app), Some(sprinting), "after the respawn");

        trigger(&mut app, connection, &login);
        app.world_mut().run_schedule(FixedUpdate);
        assert_eq!(speed(&app), Some(sprinting), "after the second login");
    }

    fn trigger<P: Row + Encode>(app: &mut App, connection: Entity, packet: &P) {
        let mut data = Vec::new();
        packet.encode(&mut data).expect("the test packet encodes");
        app.world_mut()
            .trigger(Received::<P>::new(connection, data.into()));
        app.world_mut().flush();
    }

    fn spawn_info() -> PlayerSpawnInfo {
        PlayerSpawnInfo::new(
            Id::checked(0, 1).expect("the dimension type id is in range"),
            OVERWORLD.into(),
            GameMode::Survival,
        )
    }

    fn login() -> ClientboundLogin {
        ClientboundLogin {
            player_id: 7,
            hardcore: false,
            dimensions: Vec::new(),
            max_players: VarInt(20),
            chunk_radius: VarInt(8),
            simulation_distance: VarInt(8),
            reduced_debug_info: false,
            show_death_screen: true,
            do_limited_crafting: false,
            player_spawn_info: spawn_info(),
            online_mode: false,
            enforces_secure_chat: false,
        }
    }

    #[test]
    fn forward_moves_along_the_horizontal_look_direction() {
        let forward = MovementInput {
            forward: true,
            ..MovementInput::EMPTY
        };
        for yaw in [0.0, 90.0, -139.9493, 180.0] {
            let moved = tick(Sprint::default(), FlyingSpeed(1.0), yaw, forward);
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
        apply_server_teleport(&mut transform, &mut velocity, teleport);
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

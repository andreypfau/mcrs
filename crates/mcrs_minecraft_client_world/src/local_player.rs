use bevy::math::DVec3;
use bevy::prelude::*;
use mcrs_minecraft_engine_world::entity::physics::{
    OldTransform, Rotation, Transform as PhysicsTransform, Velocity,
};
use mcrs_minecraft_entity::SPRINT_MODIFIER;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;
use mcrs_minecraft_entity_attribute::{
    AttributeInstances, InstanceChange, InstanceEdit, InstanceModifier,
};
use mcrs_minecraft_network::ConnectionState;
use mcrs_minecraft_network::client::{ClientConnection, ClientNetworkSystems};
use mcrs_minecraft_physics::{Orientation, PlayerState, Relative, Teleport, apply_teleport};
use mcrs_minecraft_protocol::{VarInt, WritePacket};
use mcrs_minecraft_protocol_game::MoveFlags;
use mcrs_minecraft_protocol_game::clientbound::{ClientboundLogin, ClientboundRespawn};
use mcrs_minecraft_protocol_game::serverbound::{
    PlayerInputFlags, ServerboundAcceptTeleportation, ServerboundClientTickEnd,
    ServerboundMovePlayerPos, ServerboundMovePlayerPosRot, ServerboundMovePlayerRot,
    ServerboundPlayerInput,
};

use crate::attribute_packets;
use crate::collision_tables::ClientPhysicsTables;
use crate::columns::ColumnStore;
use crate::health_packets;
use crate::hotbar_sync::SendSelectedSlot;
use crate::local_motion::{self, LocalMotion};
use crate::movement_input::MovementInput;
use crate::player::Player;
use crate::player_killed::{self, PlayerKilled};
use crate::player_load::{self, PlayerLoad};
use crate::player_view::{PendingTeleports, PlayerViewPlugin, ServerTeleport};
use crate::request_respawn::{self, RequestRespawn};
use crate::respawn_screen;
use crate::walk::walk;

pub const TICKS_PER_SECOND: f64 = 20.0;

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

#[derive(Component, Debug, Clone, Copy, Default)]
pub struct LastSentMovement(Option<SentMovement>);

#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LastSentInput(Option<MovementInput>);

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

#[derive(Resource, Clone, Copy)]
pub struct LookOverride {
    pub yaw: f32,
    pub pitch: f32,
}

#[derive(Resource, Clone, Copy)]
pub struct PositionOverride(pub DVec3);

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct LocalPlayerTick;

pub struct LocalPlayerPlugin;

impl Plugin for LocalPlayerPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<PlayerViewPlugin>() {
            app.add_plugins(PlayerViewPlugin);
        }
        app.init_resource::<MovementInput>()
            .add_message::<PlayerKilled>()
            .add_message::<RequestRespawn>()
            .add_observer(attribute_packets::on_login)
            .add_observer(attribute_packets::on_respawn)
            .add_observer(attribute_packets::on_update_attributes)
            .add_observer(health_packets::on_set_health)
            .add_observer(player_killed::on_player_combat_kill)
            .add_observer(health_packets::on_set_entity_data)
            .add_observer(health_packets::on_respawn)
            .add_observer(respawn_screen::on_game_event)
            .add_observer(local_motion::on_login)
            .add_observer(local_motion::on_respawn)
            .add_observer(local_motion::on_set_entity_data)
            .add_observer(player_load::start_load::<ClientboundLogin>)
            .add_observer(player_load::start_load::<ClientboundRespawn>)
            .add_observer(player_load::on_game_event)
            .add_systems(
                FixedUpdate,
                (
                    accept_teleports.before(LocalPlayerTick),
                    player_load::end_player_load
                        .run_if(player_load::player_loading)
                        .after(accept_teleports)
                        .before(SendSelectedSlot)
                        .before(LocalPlayerTick),
                    (
                        capture_old_transform,
                        local_motion::write_motion_inputs,
                        walk.run_if(resource_exists::<ColumnStore>)
                            .run_if(resource_exists::<ClientPhysicsTables>),
                        give_sprint_modifier,
                    )
                        .chain()
                        .in_set(LocalPlayerTick),
                    send_movement.after(LocalPlayerTick),
                ),
            )
            .add_systems(
                Update,
                request_respawn::send_respawn_requests
                    .run_if(on_message::<request_respawn::RequestRespawn>)
                    .after(ClientNetworkSystems::Receive)
                    .before(ClientNetworkSystems::Flush),
            );
    }
}

fn apply_server_teleport(
    state: &mut PlayerState,
    transform: &mut PhysicsTransform,
    velocity: &mut Velocity,
    teleport: &ServerTeleport,
) {
    let mut rotation = Orientation::facing(transform.rotation.yaw(), transform.rotation.pitch());
    let packet = Teleport {
        position: teleport.position,
        velocity: teleport.velocity,
        yaw: teleport.look.yaw,
        pitch: teleport.look.pitch,
        relative: Relative::from_bits_truncate(teleport.flags.bits()),
    };
    apply_teleport(state, &mut rotation, &packet);
    transform.translation = state.body.position;
    transform.rotation = Rotation::new(rotation.yaw, rotation.pitch);
    velocity.0 = state.body.velocity;
}

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::missed_set_if_neq,
        reason = "`apply_server_teleport` has already marked the motion and the transform changed, so a comparison saves nothing"
    )
)]
fn accept_teleports(
    mut commands: Commands,
    player: Single<
        (
            &mut PhysicsTransform,
            &mut OldTransform,
            &mut Velocity,
            &mut LastSentMovement,
            &mut LocalMotion,
        ),
        With<Player>,
    >,
    connection: Option<Single<(&mut ClientConnection, &mut PendingTeleports)>>,
    look: Option<Res<LookOverride>>,
    position: Option<Res<PositionOverride>>,
) {
    let Some(connection) = connection else { return };
    let (mut connection, mut pending) = connection.into_inner();
    if pending.0.is_empty() {
        return;
    }
    let (mut transform, mut old_transform, mut velocity, mut last_sent, mut motion) =
        player.into_inner();
    let mut position = position.map(|position| position.0);
    for teleport in pending.0.drain(..) {
        apply_server_teleport(&mut motion.0, &mut transform, &mut velocity, &teleport);
        if teleport.flags.is_empty() {
            if let Some(position) = position.take() {
                motion.0.body.position = position;
                transform.translation = position;
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
        if old_transform.0 != *transform {
            old_transform.0 = *transform;
        }
    }
}

fn send_movement(
    input: Res<MovementInput>,
    player: Single<
        (
            &PhysicsTransform,
            &LocalMotion,
            &mut LastSentMovement,
            &mut LastSentInput,
            &mut PlayerLoad,
        ),
        With<Player>,
    >,
    connection: Option<Single<(&mut ClientConnection, &ConnectionState)>>,
) {
    let Some(connection) = connection else { return };
    let (mut connection, state) = connection.into_inner();
    if *state != ConnectionState::Game {
        return;
    }

    let (transform, motion, mut last_sent, mut last_input, mut load) = player.into_inner();
    if *load == PlayerLoad::Loaded && last_input.0 != Some(*input) {
        last_input.0 = Some(*input);
        connection.write_packet(&ServerboundPlayerInput {
            input: input_flags(*input),
        });
    }
    let body_flags = motion.0.body.flags;
    let flags = MoveFlags::new()
        .with_on_ground(body_flags.on_ground)
        .with_horizontal_collision(body_flags.horizontal_collision);
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
    if let PlayerLoad::Loading(progress) = &mut *load {
        progress.tick_ends_sent = progress.tick_ends_sent.saturating_add(1);
    }
}

fn input_flags(input: MovementInput) -> PlayerInputFlags {
    PlayerInputFlags::new()
        .with_forward(input.forward)
        .with_backward(input.backward)
        .with_left(input.left)
        .with_right(input.right)
        .with_jump(input.jump)
        .with_shift(input.shift)
        .with_sprint(input.sprint)
}

fn capture_old_transform(player: Single<(&PhysicsTransform, &mut OldTransform), With<Player>>) {
    let (transform, mut old_transform) = player.into_inner();
    if old_transform.0 != *transform {
        old_transform.0 = *transform;
    }
}

fn give_sprint_modifier(
    mut players: Populated<(&Sprint, &mut AttributeInstances), (With<Player>, Changed<Sprint>)>,
) {
    for (sprint, mut instances) in &mut players {
        let edit = if sprint.active {
            InstanceEdit::Put(InstanceModifier::transient(&SPRINT_MODIFIER))
        } else {
            InstanceEdit::Remove(SPRINT_MODIFIER.id.to_buf())
        };
        let change = instances
            .bypass_change_detection()
            .apply(Attribute::MovementSpeed, &edit);
        if change == InstanceChange::Changed {
            instances.set_changed();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_session::{FLOOR_TOP, TestSession, login, spawn_info};
    use mcrs_minecraft_core::Look;
    use mcrs_minecraft_entity::entity_type_keys::EntityType;
    use mcrs_minecraft_game_mode::GameMode;
    use mcrs_minecraft_protocol_game::PositionFlag;
    use mcrs_minecraft_protocol_game::clientbound::{AttributeSnapshot, ClientboundRespawn};

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
        let bases = mcrs_minecraft_entity::attribute_bases(EntityType::Player)
            .expect("a player has attributes");
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
                .map_unchanged(|sprint| &mut sprint.active)
                .set_if_neq(active);
            app.update();
        };

        set_sprint(&mut app, true);
        assert_eq!(speed(&app), Some(sprinting.to_bits()));

        let _change = app
            .world_mut()
            .get_mut::<AttributeInstances>(player)
            .expect("the player has instances")
            .receive(&AttributeSnapshot {
                attribute: Attribute::MovementSpeed.into(),
                base,
                modifiers: Vec::new(),
            })
            .expect("movement_speed is in the registry");
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
        let held = MovementInput {
            forward: true,
            sprint: true,
            ..MovementInput::EMPTY
        };
        let mut session = TestSession::new(DVec3::new(8.5, FLOOR_TOP, 2.5));
        let speed = |session: &TestSession| {
            session
                .app
                .world()
                .get::<AttributeInstances>(session.player)
                .and_then(|instances| instances.value(Attribute::MovementSpeed))
                .map(f64::to_bits)
        };

        session.receive(login(GameMode::Survival));
        session.ticks(held, 1);
        assert_eq!(speed(&session), Some(sprinting));

        session.receive(ClientboundRespawn {
            player_spawn_info: spawn_info(GameMode::Survival),
            data_to_keep: 0,
        });
        session.ticks(held, 1);
        assert_eq!(speed(&session), Some(sprinting), "after the respawn");

        session.receive(login(GameMode::Survival));
        session.ticks(held, 1);
        assert_eq!(speed(&session), Some(sprinting), "after the second login");
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
            translation: DVec3::ZERO,
            rotation: Rotation::new(45.0, 10.0),
        };
        let mut state = PlayerState::new(DVec3::new(10.0, 64.0, -5.0));
        state.body.velocity = DVec3::new(1.0, 0.0, -1.0);
        let mut velocity = Velocity(DVec3::ZERO);
        apply_server_teleport(&mut state, &mut transform, &mut velocity, teleport);
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

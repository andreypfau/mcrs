use bevy_app::{FixedPostUpdate, FixedUpdate, Plugin};
use bevy_ecs::component::Component;
use bevy_ecs::prelude::{
    Changed, DetectChangesMut, Entity, Message, MessageReader, MessageWriter, Mut, On, Query,
};
use bevy_math::{DVec3, Quat};
use mcrs_engine::entity::physics::Transform;
use mcrs_network::ServerSideConnection;
use mcrs_network::event::ReceivedPacketEvent;
use mcrs_protocol::packets::game::clientbound::ClientboundPlayerPosition;
use mcrs_protocol::packets::game::serverbound::{
    ServerboundMovePlayerPos, ServerboundMovePlayerPosRot, ServerboundMovePlayerRot,
    ServerboundMovePlayerStatusOnly,
};
use mcrs_protocol::{Look, MoveFlags, PositionFlag, WritePacket};

pub struct MovementPlugin;

impl Plugin for MovementPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_observer(handle_move_packets);
        app.add_message::<PlayerMovement>();
        app.add_systems(FixedUpdate, process_movement);
        app.add_systems(FixedPostUpdate, teleport);
    }
}

#[derive(Component, Debug)]
pub struct TeleportState {
    /// Counts up as teleports are made.
    teleport_id_counter: u32,
    /// The number of pending client teleports that have yet to receive a
    /// confirmation. Inbound client position packets should be ignored while
    /// this is nonzero.
    pending_teleports: u32,
    synced_transform: Transform,
}

impl TeleportState {
    pub fn teleport_id_counter(&self) -> u32 {
        self.teleport_id_counter
    }

    pub fn pending_teleports(&self) -> u32 {
        self.pending_teleports
    }
}

impl Default for TeleportState {
    fn default() -> Self {
        Self {
            teleport_id_counter: 0,
            pending_teleports: 0,
            // Set initial synced pos and look to NaN so a teleport always happens when first
            // joining.
            synced_transform: Transform::default(),
        }
    }
}

fn handle_move_packets(on: On<ReceivedPacketEvent>, mut writer: MessageWriter<PlayerMovement>) {
    let e = on.entity;
    if let Some(p) = on.decode::<ServerboundMovePlayerPos>() {
        writer.write(PlayerMovement::new(
            e,
            Some(p.position.into()),
            None,
            p.flags,
        ));
    } else if let Some(p) = on.decode::<ServerboundMovePlayerPosRot>() {
        let m = PlayerMovement::new(e, Some(p.position.into()), Some(p.look.into()), p.flags);
        writer.write(m);
    } else if let Some(p) = on.decode::<ServerboundMovePlayerRot>() {
        let m = PlayerMovement::new(e, None, Some(p.look.into()), p.flags);
        writer.write(m);
    } else if let Some(p) = on.decode::<ServerboundMovePlayerStatusOnly>() {
        writer.write(PlayerMovement::new(e, None, None, p.flags));
    }
}

#[derive(Message)]
pub struct PlayerMovement {
    entity: Entity,
    position: Option<DVec3>,
    look: Option<Quat>,
    flags: MoveFlags,
}

impl PlayerMovement {
    pub fn new(
        entity: Entity,
        position: Option<DVec3>,
        look: Option<Quat>,
        flags: MoveFlags,
    ) -> Self {
        Self {
            entity,
            position,
            look,
            flags,
        }
    }
}

fn process_movement(
    mut reader: MessageReader<PlayerMovement>,
    mut query: Query<(Mut<TeleportState>, Mut<Transform>)>,
) {
    const MAX_XZ: f64 = 30_000_000.0;
    const MAX_Y: f64 = 20_000_000.0;
    const MAX_POS: DVec3 = DVec3::new(MAX_XZ, MAX_Y, MAX_XZ);
    const MIN_POS: DVec3 = DVec3::new(-MAX_XZ, -MAX_Y, -MAX_XZ);

    reader.read().for_each(|m| {
        let Ok((mut state, mut transform)) = query.get_mut(m.entity) else {
            return;
        };
        m.position.map(|mut p| {
            transform.set_if_neq(transform.with_translation(p.clamp(MIN_POS, MAX_POS)));
        });
        m.look.map(|l| {
            transform.set_if_neq(transform.with_rotation(l));
        });
        state.synced_transform = *transform;
    })
}

#[allow(clippy::type_complexity)]
fn teleport(
    mut clients: Query<
        (&mut ServerSideConnection, &mut TeleportState, &Transform),
        Changed<Transform>,
    >,
) {
    for (mut client, mut state, transform) in &mut clients {
        let changed_pos = transform.translation != state.synced_transform.translation;
        let changed_y_rot = transform.rotation.y != state.synced_transform.rotation.y;
        let changed_x_rot = transform.rotation.x != state.synced_transform.rotation.x;

        if changed_pos || changed_y_rot || changed_x_rot {
            state.synced_transform = *transform;

            let flags = {
                let mut f = Vec::new();
                if !changed_pos {
                    f.push(PositionFlag::X);
                    f.push(PositionFlag::Y);
                    f.push(PositionFlag::Z);
                }
                if !changed_y_rot {
                    f.push(PositionFlag::YRot);
                }
                if !changed_x_rot {
                    f.push(PositionFlag::XRot);
                }
                f
            };

            client.write_packet(&ClientboundPlayerPosition {
                teleport_id: (state.teleport_id_counter as i32).into(),
                position: if changed_pos {
                    transform.translation
                } else {
                    DVec3::ZERO
                },
                velocity: Default::default(),
                look: Look {
                    yaw: if changed_y_rot {
                        transform.rotation.y
                    } else {
                        0.0
                    },
                    pitch: if changed_x_rot {
                        transform.rotation.x
                    } else {
                        0.0
                    },
                },
                flags,
            });

            state.pending_teleports = state.pending_teleports.wrapping_add(1);
            state.teleport_id_counter = state.teleport_id_counter.wrapping_add(1);
        }
    }
}

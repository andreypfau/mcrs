use bevy_app::{FixedPostUpdate, FixedUpdate, Plugin};
use bevy_ecs::bundle::Bundle;
use bevy_ecs::component::Component;
use bevy_ecs::prelude::{Changed, DetectChangesMut, Entity, Message, MessageReader, MessageWriter, Mut, On, Or, Query};
use mcrs_network::ServerSideConnection;
use mcrs_network::event::ReceivedPacketEvent;
use mcrs_protocol::math::DVec3;
use mcrs_protocol::packets::game::clientbound::ClientboundPlayerPosition;
use mcrs_protocol::packets::game::serverbound::{
    ServerboundMovePlayerPos, ServerboundMovePlayerPosRot, ServerboundMovePlayerRot,
    ServerboundMovePlayerStatusOnly,
};
use mcrs_protocol::{Look, MoveFlags, Position, PositionFlag, WritePacket};

pub struct MovementPlugin;

impl Plugin for MovementPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_observer(handle_move_packets);
        app.add_message::<PlayerMovement>();
        app.add_systems(FixedUpdate, process_movement);
        app.add_systems(FixedPostUpdate, teleport);
    }
}

#[derive(Bundle, Default)]
pub struct MovementBundle {
    pub teleport_state: TeleportState,
    pub position: Position,
    pub look: Look,
}

#[derive(Component, Debug)]
pub struct TeleportState {
    /// Counts up as teleports are made.
    teleport_id_counter: u32,
    /// The number of pending client teleports that have yet to receive a
    /// confirmation. Inbound client position packets should be ignored while
    /// this is nonzero.
    pending_teleports: u32,
    synced_pos: Position,
    synced_look: Look,
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
            synced_pos: Position::from(DVec3::NAN),
            synced_look: Look {
                yaw: f32::NAN,
                pitch: f32::NAN,
            },
        }
    }
}

fn handle_move_packets(on: On<ReceivedPacketEvent>, mut writer: MessageWriter<PlayerMovement>) {
    let e = on.entity;
    if let Some(p) = on.decode::<ServerboundMovePlayerPos>() {
        writer.write(PlayerMovement::new(e, Some(p.position), None, p.flags));
    } else if let Some(p) = on.decode::<ServerboundMovePlayerPosRot>() {
        writer.write(PlayerMovement::new(e, Some(p.position), Some(p.look), p.flags));
    } else if let Some(p) = on.decode::<ServerboundMovePlayerRot>() {
        writer.write(PlayerMovement::new(e, None, Some(p.look), p.flags));
    } else if let Some(p) = on.decode::<ServerboundMovePlayerStatusOnly>() {
        writer.write(PlayerMovement::new(e, None, None, p.flags));
    }
}

#[derive(Message)]
pub struct PlayerMovement {
    entity: Entity,
    position: Option<Position>,
    look: Option<Look>,
    flags: MoveFlags,
}

impl PlayerMovement {
    pub fn new(
        entity: Entity,
        position: Option<Position>,
        look: Option<Look>,
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
    mut query: Query<(Mut<TeleportState>, Mut<Position>, Mut<Look>)>
) {
    const MAX_XZ: f64 = 30_000_000.0;
    const MAX_Y: f64 = 20_000_000.0;
    const MAX_POS: Position = Position::new(MAX_XZ, MAX_Y, MAX_XZ);
    const MIN_POS: Position = Position::new(-MAX_XZ, -MAX_Y, -MAX_XZ);

    reader.read().for_each(|m| {
        let Ok((mut state, mut pos, mut look)) = query.get_mut(m.entity) else {
            return;
        };
        m.position.map(|mut p| {
            p = Position::from(p.clamp(*MIN_POS, *MAX_POS));

            pos.set_if_neq(p);
            state.synced_pos = p;
        });
        m.look.map(|l| {
            look.set_if_neq(l);
            state.synced_look = l;
        });
    })
}

#[allow(clippy::type_complexity)]
fn teleport(
    mut clients: Query<
        (
            &mut ServerSideConnection,
            &mut TeleportState,
            &Position,
            &Look,
        ),
        Or<(Changed<Position>, Changed<Look>)>,
    >,
) {
    for (mut client, mut state, pos, look) in &mut clients {
        let changed_pos = *pos != state.synced_pos;
        let changed_yaw = look.yaw != state.synced_look.yaw;
        let changed_pitch = look.pitch != state.synced_look.pitch;

        if changed_pos || changed_yaw || changed_pitch {
            state.synced_pos = *pos;
            state.synced_look = *look;

            let flags = {
                let mut f = Vec::new();
                if !changed_pos {
                    f.push(PositionFlag::X);
                    f.push(PositionFlag::Y);
                    f.push(PositionFlag::Z);
                }
                if !changed_yaw {
                    f.push(PositionFlag::YRot);
                }
                if !changed_pitch {
                    f.push(PositionFlag::XRot);
                }
                f
            };

            client.write_packet(&ClientboundPlayerPosition {
                teleport_id: (state.teleport_id_counter as i32).into(),
                position: if changed_pos {
                    *pos
                } else {
                    Position::from(DVec3::ZERO)
                },
                velocity: Default::default(),
                look: Look {
                    yaw: if changed_yaw { look.yaw } else { 0.0 },
                    pitch: if changed_pitch { look.pitch } else { 0.0 },
                },
                flags,
            });

            state.pending_teleports = state.pending_teleports.wrapping_add(1);
            state.teleport_id_counter = state.teleport_id_counter.wrapping_add(1);
        }
    }
}

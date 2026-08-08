use bevy_app::{FixedPostUpdate, FixedUpdate, Plugin};
use bevy_ecs::component::Component;
use bevy_ecs::prelude::{
    Changed, DetectChangesMut, Entity, Message, MessageReader, MessageWriter, Mut, On, Query, With,
};
use bevy_math::{DVec3, Quat};
use mcrs_engine::entity::physics::Transform;
use mcrs_engine::session::PlayerSession;
use mcrs_network::event::ReceivedPacketEvent;
use mcrs_protocol::packets::game::serverbound::{
    ServerboundAcceptTeleportation, ServerboundMovePlayerPos, ServerboundMovePlayerPosRot,
    ServerboundMovePlayerRot, ServerboundMovePlayerStatusOnly,
};
use mcrs_protocol::MoveFlags;

use crate::world::bus::{OutboundPlayerPacket, PacketPayload, PacketPriority, PacketTarget};
use crate::world::entity::player::HostAnchor;

pub struct MovementPlugin;

impl Plugin for MovementPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_observer(handle_move_packets);
        app.add_observer(handle_accept_teleportation);
        app.add_message::<PlayerMovement>();
        app.add_systems(FixedUpdate, process_movement);
        app.add_systems(FixedPostUpdate, teleport);
    }
}

#[derive(Component, Debug)]
#[derive(Default)]
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

    /// Claim the next teleport id from the shared sequence and register a
    /// pending confirmation. Used by both the login position sync and the
    /// server-authoritative `teleport` system so login ids draw from the same
    /// monotonic counter and can never alias a later teleport id.
    pub fn next_teleport_id(&mut self) -> i32 {
        let id = self.teleport_id_counter as i32;
        self.teleport_id_counter = self.teleport_id_counter.wrapping_add(1);
        self.pending_teleports = self.pending_teleports.wrapping_add(1);
        id
    }

    /// Teleport id reserved for the login position sync. The spawn consumer
    /// sends `PlayerPosition` with this id at login, so the runtime counter is
    /// initialized past it (see [`Self::after_login`]) and the first
    /// server-authoritative teleport draws the next id without aliasing.
    pub const LOGIN_TELEPORT_ID: i32 = 0;

    /// Initial state for a freshly spawned player whose login position sync has
    /// already claimed [`Self::LOGIN_TELEPORT_ID`]. The counter starts past the
    /// login id and one confirmation is outstanding for that login teleport.
    pub fn after_login() -> Self {
        Self {
            teleport_id_counter: (Self::LOGIN_TELEPORT_ID as u32).wrapping_add(1),
            pending_teleports: 1,
            ..Default::default()
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
        if let Some(p) = m.position { transform.set_if_neq(transform.with_translation(p.clamp(MIN_POS, MAX_POS))); }
        if let Some(l) = m.look { transform.set_if_neq(transform.with_rotation(l)); }
        state.synced_transform = *transform;
    })
}

#[allow(clippy::type_complexity)]
fn teleport(
    mut clients: Query<
        (&HostAnchor, &mut TeleportState, &Transform),
        (Changed<Transform>, With<HostAnchor>),
    >,
    mut packet_writer: MessageWriter<OutboundPlayerPacket>,
) {
    for (anchor, mut state, transform) in &mut clients {
        let changed_pos = transform.translation != state.synced_transform.translation;
        let changed_y_rot = transform.rotation.y != state.synced_transform.rotation.y;
        let changed_x_rot = transform.rotation.x != state.synced_transform.rotation.x;

        if changed_pos || changed_y_rot || changed_x_rot {
            state.synced_transform = *transform;

            let teleport_id = state.next_teleport_id();
            packet_writer.write(OutboundPlayerPacket {
                target: PacketTarget::SinglePlayer(anchor.0),
                priority: PacketPriority::Critical,
                data: PacketPayload::PlayerPosition {
                    teleport_id,
                    position: transform.translation,
                },
                session: PlayerSession(0),
                epoch: 0,
            });
        }
    }
}

/// Decrement the pending-teleport counter when the client confirms a teleport.
/// The server increments `pending_teleports` for every server-authoritative
/// position it sends (login sync and each `teleport` emit); without this the
/// counter would grow without bound and any gate on it would block the player
/// permanently. `event.entity` is the in-dim player entity, matching the
/// `TeleportState` owner.
fn handle_accept_teleportation(
    event: On<ReceivedPacketEvent>,
    mut players: Query<&mut TeleportState>,
) {
    if event.decode::<ServerboundAcceptTeleportation>().is_none() {
        return;
    }
    if let Ok(mut state) = players.get_mut(event.entity) {
        state.pending_teleports = state.pending_teleports.saturating_sub(1);
    }
}

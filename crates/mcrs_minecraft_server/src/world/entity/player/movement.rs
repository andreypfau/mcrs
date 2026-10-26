use bevy_app::{FixedPostUpdate, FixedUpdate, Plugin};
use bevy_ecs::component::Component;
use bevy_ecs::prelude::{
    Changed, DetectChangesMut, Entity, Message, MessageReader, MessageWriter, Mut, On, Query, With,
};
use bevy_ecs::schedule::IntoScheduleConfigs;
use bevy_math::DVec3;
use mcrs_minecraft_engine::bus::to;
use mcrs_minecraft_level::entity::physics::{Rotation, Transform};
use mcrs_minecraft_level::tick::TickSet;
use mcrs_minecraft_level::voxel_update::VoxelUpdateSet;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol::Look;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundPlayerPosition;
use mcrs_minecraft_protocol::packets::game::serverbound::{
    ServerboundAcceptTeleportation, ServerboundMovePlayerPos, ServerboundMovePlayerPosRot,
    ServerboundMovePlayerRot, ServerboundMovePlayerStatusOnly,
};

use crate::world::entity::player::HostAnchor;
use mcrs_minecraft_engine::bus::{OutboundPlayerPacket, PacketPayload};

pub struct MovementPlugin;

impl Plugin for MovementPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_observer(handle_move_pos);
        app.add_observer(handle_move_pos_rot);
        app.add_observer(handle_move_rot);
        app.add_observer(handle_move_status_only);
        app.add_observer(handle_accept_teleportation);
        app.add_message::<PlayerMovement>();
        app.add_systems(FixedUpdate, process_movement.in_set(TickSet::Input));
        app.add_systems(
            FixedPostUpdate,
            teleport.before(VoxelUpdateSet::NetworkSync),
        );
    }
}

#[derive(Component, Debug, Default)]
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

    /// A move the caller sends to the client itself: the next teleport id, after which the
    /// `teleport` system treats `transform` as already sent.
    pub fn sent(&mut self, transform: Transform) -> i32 {
        self.synced_transform = transform;
        self.next_teleport_id()
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

fn handle_move_pos(
    on: On<Received<ServerboundMovePlayerPos>>,
    mut writer: MessageWriter<PlayerMovement>,
) {
    if let Some(p) = on.packet() {
        writer.write(PlayerMovement {
            entity: on.entity,
            position: Some(p.position.into()),
            look: None,
        });
    }
}

fn handle_move_pos_rot(
    on: On<Received<ServerboundMovePlayerPosRot>>,
    mut writer: MessageWriter<PlayerMovement>,
) {
    if let Some(p) = on.packet() {
        writer.write(PlayerMovement {
            entity: on.entity,
            position: Some(p.position.into()),
            look: Some(Rotation::new(p.look.yaw, p.look.pitch)),
        });
    }
}

fn handle_move_rot(
    on: On<Received<ServerboundMovePlayerRot>>,
    mut writer: MessageWriter<PlayerMovement>,
) {
    if let Some(p) = on.packet() {
        writer.write(PlayerMovement {
            entity: on.entity,
            position: None,
            look: Some(Rotation::new(p.look.yaw, p.look.pitch)),
        });
    }
}

fn handle_move_status_only(
    on: On<Received<ServerboundMovePlayerStatusOnly>>,
    mut writer: MessageWriter<PlayerMovement>,
) {
    if on.packet().is_some() {
        writer.write(PlayerMovement {
            entity: on.entity,
            position: None,
            look: None,
        });
    }
}

#[derive(Message)]
pub struct PlayerMovement {
    entity: Entity,
    position: Option<DVec3>,
    look: Option<Rotation>,
}

pub(crate) const MAX_XZ: f64 = 30_000_000.0;
pub(crate) const MAX_Y: f64 = 20_000_000.0;

pub(crate) fn process_movement(
    mut reader: MessageReader<PlayerMovement>,
    mut query: Query<(Mut<TeleportState>, Mut<Transform>)>,
) {
    const MAX_POS: DVec3 = DVec3::new(MAX_XZ, MAX_Y, MAX_XZ);
    const MIN_POS: DVec3 = DVec3::new(-MAX_XZ, -MAX_Y, -MAX_XZ);

    reader.read().for_each(|m| {
        let Ok((mut state, mut transform)) = query.get_mut(m.entity) else {
            return;
        };
        if let Some(p) = m.position {
            transform.set_if_neq(transform.with_translation(p.clamp(MIN_POS, MAX_POS)));
        }
        if let Some(l) = m.look {
            transform.set_if_neq(transform.with_rotation(l));
        }
        state.synced_transform = *transform;
    })
}

fn teleport(
    mut clients: Query<
        (&HostAnchor, &mut TeleportState, &Transform),
        (Changed<Transform>, With<HostAnchor>),
    >,
    mut packet_writer: MessageWriter<OutboundPlayerPacket>,
) {
    for (anchor, mut state, transform) in &mut clients {
        let changed_pos = transform.translation != state.synced_transform.translation;
        let changed_rot = transform.rotation != state.synced_transform.rotation;

        if changed_pos || changed_rot {
            state.synced_transform = *transform;

            let teleport_id = state.next_teleport_id();
            packet_writer.write(
                to(
                    anchor.0,
                    PacketPayload::PlayerPosition(ClientboundPlayerPosition {
                        teleport_id: VarInt(teleport_id),
                        position: transform.translation,
                        velocity: DVec3::ZERO,
                        look: Look::default(),
                        flags: mcrs_minecraft_protocol::PositionFlags::default(),
                    }),
                )
                .critical(),
            );
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
    event: On<Received<ServerboundAcceptTeleportation>>,
    mut players: Query<&mut TeleportState>,
) {
    if event.packet().is_none() {
        return;
    }
    if let Ok(mut state) = players.get_mut(event.entity) {
        state.pending_teleports = state.pending_teleports.saturating_sub(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::message::Messages;
    use bevy_ecs::system::RunSystemOnce;
    use bevy_ecs::world::World;
    use mcrs_minecraft_engine::bus::PacketTarget;

    #[test]
    fn a_server_move_sends_one_position_with_the_next_teleport_id_and_a_synced_one_none() {
        let mut world = World::new();
        world.init_resource::<Messages<OutboundPlayerPacket>>();
        let moved_anchor = world.spawn_empty().id();
        let moved = world
            .spawn((
                HostAnchor(moved_anchor),
                Transform::default().with_translation(DVec3::new(1.0, 0.0, 0.0)),
                TeleportState::default(),
            ))
            .id();
        let synced_anchor = world.spawn_empty().id();
        let synced = world
            .spawn((
                HostAnchor(synced_anchor),
                Transform::default(),
                TeleportState::default(),
            ))
            .id();

        world.run_system_once(teleport).unwrap();

        let sent: Vec<_> = world
            .resource_mut::<Messages<OutboundPlayerPacket>>()
            .drain()
            .collect();
        assert_eq!(sent.len(), 1, "only the moved player is told");
        assert!(
            matches!(sent[0].target, PacketTarget::SinglePlayer(anchor) if anchor == moved_anchor)
        );
        let PacketPayload::PlayerPosition(position) = &sent[0].data else {
            panic!("a move is sent as a player position");
        };
        assert_eq!(position.teleport_id.0, 0);
        assert_eq!(position.position, DVec3::new(1.0, 0.0, 0.0));

        let state = world.get::<TeleportState>(moved).unwrap();
        assert_eq!(
            (state.pending_teleports(), state.teleport_id_counter()),
            (1, 1)
        );
        let state = world.get::<TeleportState>(synced).unwrap();
        assert_eq!(
            (state.pending_teleports(), state.teleport_id_counter()),
            (0, 0)
        );
    }
}

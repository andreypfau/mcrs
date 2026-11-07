//! The bounds of a teleport and the teleport state of a player.

use bevy_ecs::component::Component;
use bevy_ecs::resource::Resource;
use bevy_math::DVec3;
use mcrs_minecraft_engine_world::entity::physics::{Rotation, Transform};
use mcrs_minecraft_physics::{
    PlayerRotation, PlayerState, Relative, SineTable, Teleport, apply_teleport,
};
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol_game::clientbound::ClientboundPlayerPosition;
use mcrs_minecraft_protocol_game::{PositionFlag, PositionFlags};

use crate::coordinator::clock::GameTime;
use crate::movement_checks::MovementChecks;

/// The horizontal bound, in blocks, of a position a player can reach.
pub const MAX_XZ_BLOCKS: i32 = 30_000_000;
/// [`MAX_XZ_BLOCKS`] as a coordinate.
pub const MAX_XZ: f64 = MAX_XZ_BLOCKS as f64;
/// The vertical bound of a position a player can reach.
pub const MAX_Y: f64 = 20_000_000.0;

/// The largest distance, in blocks, between the position of an accept and the destination of its
/// teleport. The client computes the destination in its own arithmetic.
pub const OFFSET_THRESHOLD: f64 = 0.001;

/// A move resends the pending teleport once more than this many ticks passed after it was sent.
pub const RESEND_INTERVAL: i64 = 20;

/// The server-side state of the teleports of a player: the newest id it sent, the teleport the
/// client did not confirm yet, and the transform the client last knew.
#[derive(Component, Clone, Debug, Default)]
pub struct TeleportState {
    /// The id of the newest teleport; 0 before the first one.
    last_id: i32,
    pending: Option<PendingTeleport>,
    synced_transform: Transform,
}

/// A teleport the client did not confirm yet. Only the newest teleport can be pending: a newer
/// one replaces it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PendingTeleport {
    pub id: i32,
    pub destination: Transform,
    pub sent_at: GameTime,
}

/// What an accept does to the pending teleport.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AcceptOutcome {
    /// The accept confirmed the pending teleport; moves apply again.
    Confirmed,
    /// The accept names no pending teleport, or an older one; nothing changes.
    Ignored,
    /// The accept names the pending teleport at a position away from its destination. The
    /// teleport stays pending, and the caller sends a correction to `destination`.
    Mismatch { destination: Transform },
}

impl TeleportState {
    /// The teleport the client did not confirm yet.
    pub fn pending(&self) -> Option<&PendingTeleport> {
        self.pending.as_ref()
    }

    /// A teleport to `destination` that the caller sends to the client itself, at `now`: the id
    /// of the packet, after which the `teleport` system treats `destination` as already sent.
    pub fn sent(&mut self, destination: Transform, now: GameTime) -> i32 {
        self.last_id = self
            .last_id
            .checked_add(1)
            .filter(|id| *id != i32::MAX)
            .unwrap_or(0);
        self.pending = Some(PendingTeleport {
            id: self.last_id,
            destination,
            sent_at: now,
        });
        self.synced_transform = destination;
        self.last_id
    }

    /// The destination to send again when a move arrives at `now` while a teleport is pending.
    pub fn resend_due(&self, now: GameTime) -> Option<Transform> {
        self.pending
            .filter(|pending| now.0.saturating_sub(pending.sent_at.0) > RESEND_INTERVAL)
            .map(|pending| pending.destination)
    }

    /// Applies an accept of `id` at `position`. Without the movement checks, an accept of the
    /// pending id confirms it at any position.
    pub fn accept(&mut self, id: i32, position: DVec3, checks: MovementChecks) -> AcceptOutcome {
        let Some(pending) = self.pending.filter(|pending| pending.id == id) else {
            return AcceptOutcome::Ignored;
        };
        if checks == MovementChecks::Off
            || position.distance(pending.destination.translation) < OFFSET_THRESHOLD
        {
            self.pending = None;
            AcceptOutcome::Confirmed
        } else {
            AcceptOutcome::Mismatch {
                destination: pending.destination,
            }
        }
    }

    /// The transform the client last sent or received.
    pub fn synced_transform(&self) -> Transform {
        self.synced_transform
    }

    /// The client reported `transform`, so the `teleport` system does not send it back.
    pub fn sync(&mut self, transform: Transform) {
        self.synced_transform = transform;
    }
}

/// The sine table of the client, which a teleport that rotates the velocity reads.
#[derive(Resource, Default)]
pub struct ClientSine(pub SineTable);

/// A teleport from `known`, the transform the client has, to `destination`. The position is
/// absolute. The client keeps its velocity, as the server does not hold one, and its rotation
/// when the rotation does not change.
#[must_use]
pub fn teleport_to(known: Transform, destination: Transform) -> Teleport {
    let mut relative = Relative::VELOCITY_X | Relative::VELOCITY_Y | Relative::VELOCITY_Z;
    let (yaw, pitch) = if destination.rotation == known.rotation {
        relative |= Relative::YAW | Relative::PITCH;
        (0.0, 0.0)
    } else {
        (destination.rotation.yaw(), destination.rotation.pitch())
    };
    Teleport {
        position: destination.translation,
        velocity: DVec3::ZERO,
        yaw,
        pitch,
        relative,
    }
}

/// The transform that the client computes from `known` for `teleport`.
#[must_use]
pub fn landing(sine: &SineTable, known: Transform, teleport: &Teleport) -> Transform {
    let mut state = PlayerState::new(known.translation);
    let mut rotation = PlayerRotation {
        yaw: known.rotation.yaw(),
        pitch: known.rotation.pitch(),
    };
    apply_teleport(sine, &mut state, &mut rotation, teleport);
    Transform {
        translation: state.position,
        rotation: Rotation::new(rotation.yaw, rotation.pitch),
    }
}

/// The `player_position` packet of `teleport` with the id `teleport_id`.
#[must_use]
pub fn position_packet(teleport_id: i32, teleport: &Teleport) -> ClientboundPlayerPosition {
    const FLAGS: [(Relative, PositionFlag); 9] = [
        (Relative::X, PositionFlag::X),
        (Relative::Y, PositionFlag::Y),
        (Relative::Z, PositionFlag::Z),
        (Relative::YAW, PositionFlag::YRot),
        (Relative::PITCH, PositionFlag::XRot),
        (Relative::VELOCITY_X, PositionFlag::DeltaX),
        (Relative::VELOCITY_Y, PositionFlag::DeltaY),
        (Relative::VELOCITY_Z, PositionFlag::DeltaZ),
        (Relative::ROTATE_VELOCITY, PositionFlag::RotateDelta),
    ];
    let flags: PositionFlags = FLAGS
        .into_iter()
        .filter(|(relative, _)| teleport.relative.contains(*relative))
        .map(|(_, flag)| flag)
        .collect();
    ClientboundPlayerPosition {
        teleport_id: VarInt(teleport_id),
        position: teleport.position,
        velocity: teleport.velocity,
        look: mcrs_minecraft_core::Look {
            yaw: teleport.yaw,
            pitch: teleport.pitch,
        },
        flags,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(x: f64) -> Transform {
        Transform::default().with_translation(DVec3::new(x, 64.0, 0.0))
    }

    #[test]
    fn a_teleport_that_keeps_the_rotation_sends_it_relative_and_keeps_the_velocity() {
        let known = at(0.0).with_rotation(Rotation::new(720.0, 10.0));
        let destination = at(5.0).with_rotation(known.rotation);
        let packet = position_packet(7, &teleport_to(known, destination));
        let relative: PositionFlags = [
            PositionFlag::YRot,
            PositionFlag::XRot,
            PositionFlag::DeltaX,
            PositionFlag::DeltaY,
            PositionFlag::DeltaZ,
        ]
        .into_iter()
        .collect();
        assert_eq!(
            (packet.teleport_id.0, packet.position, packet.velocity),
            (7, DVec3::new(5.0, 64.0, 0.0), DVec3::ZERO),
            "MVC-RSP-001: an absolute position"
        );
        assert_eq!(
            (packet.look.yaw, packet.look.pitch, packet.flags),
            (0.0, 0.0, relative),
            "MVC-RSP-001: yaw and pitch 0 with both rotation bits"
        );
    }

    #[test]
    fn a_turn_sends_the_raw_rotation_and_the_client_lands_on_the_server_transform() {
        let sine = SineTable::new();
        let known = at(0.0);
        let destination = at(5.0).with_rotation(Rotation::new(720.0, -30.0));
        let teleport = teleport_to(known, destination);
        let packet = position_packet(1, &teleport);
        assert_eq!((packet.look.yaw, packet.look.pitch), (720.0, -30.0));
        assert!(!packet.flags.contains(PositionFlag::YRot));
        assert_eq!(landing(&sine, known, &teleport), destination, "PHY-SRV-010");
    }

    #[test]
    fn the_first_teleport_has_id_1_and_each_later_one_the_next_id() {
        let mut state = TeleportState::default();
        let ids = [
            state.sent(at(0.0), GameTime(0)),
            state.sent(at(1.0), GameTime(0)),
            state.sent(at(1.0), GameTime(0)),
        ];
        assert_eq!(ids, [1, 2, 3], "TP-CONFIRM-ID-001, TP-CONFIRM-ID-002");

        let mut state = TeleportState {
            last_id: i32::MAX - 2,
            ..TeleportState::default()
        };
        let ids = [
            state.sent(at(0.0), GameTime(0)),
            state.sent(at(0.0), GameTime(0)),
        ];
        assert_eq!(ids, [i32::MAX - 1, 0], "TP-CONFIRM-ID-003");
    }

    #[test]
    fn only_an_accept_of_the_newest_id_at_the_destination_confirms() {
        let mut state = TeleportState::default();
        let older = state.sent(at(0.0), GameTime(0));
        let newest = state.sent(at(10.0), GameTime(0));
        let destination = DVec3::new(10.0, 64.0, 0.0);

        assert_eq!(
            state.accept(older, destination, MovementChecks::On),
            AcceptOutcome::Ignored
        );
        assert_eq!(
            state.accept(99, destination, MovementChecks::On),
            AcceptOutcome::Ignored
        );
        assert_eq!(
            state.accept(newest, destination + DVec3::X * 5.0, MovementChecks::On),
            AcceptOutcome::Mismatch {
                destination: at(10.0)
            },
            "MVC-LAT-008: an accept away from the destination is a correction",
        );
        assert!(state.pending().is_some(), "TP-CONFIRM-ACC-002");
        assert_eq!(
            state.accept(newest, destination + DVec3::X * 0.0005, MovementChecks::On),
            AcceptOutcome::Confirmed
        );
        assert!(state.pending().is_none());
        assert_eq!(
            state.accept(newest, destination, MovementChecks::On),
            AcceptOutcome::Ignored,
            "nothing is pending"
        );
    }

    #[test]
    fn a_move_resends_the_pending_teleport_once_more_than_20_ticks_passed() {
        let mut state = TeleportState::default();
        state.sent(at(3.0), GameTime(100));
        assert_eq!(state.resend_due(GameTime(120)), None, "TP-CONFIRM-RES-006");
        assert_eq!(
            state.resend_due(GameTime(121)),
            Some(at(3.0)),
            "TP-CONFIRM-RES-002, TP-CONFIRM-RES-006"
        );
    }
}

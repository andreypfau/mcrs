//! The bounds of a teleport and the teleport state of a player.

use bevy_ecs::component::Component;
use mcrs_minecraft_engine_world::entity::physics::Transform;

/// The horizontal bound, in blocks, of a position a player can reach.
pub const MAX_XZ_BLOCKS: i32 = 30_000_000;
/// [`MAX_XZ_BLOCKS`] as a coordinate.
pub const MAX_XZ: f64 = MAX_XZ_BLOCKS as f64;
/// The vertical bound of a position a player can reach.
pub const MAX_Y: f64 = 20_000_000.0;

/// The server-side state of the teleports of a player: the ids it sent and the client did not
/// confirm yet, and the transform the client last knew.
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
    /// The id the next teleport gets.
    pub fn teleport_id_counter(&self) -> u32 {
        self.teleport_id_counter
    }

    /// The teleports the client did not confirm yet.
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

    /// The transform the client last sent or received.
    pub fn synced_transform(&self) -> Transform {
        self.synced_transform
    }

    /// The client reported `transform`, so the `teleport` system does not send it back.
    pub fn sync(&mut self, transform: Transform) {
        self.synced_transform = transform;
    }

    /// The client confirmed one pending teleport.
    pub fn confirm(&mut self) {
        self.pending_teleports = self.pending_teleports.saturating_sub(1);
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

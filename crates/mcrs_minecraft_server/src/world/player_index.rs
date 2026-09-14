use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::resource::Resource;
use rustc_hash::FxHashMap;
use smallvec::SmallVec;

use crate::world::bus::InboundPlayerPacket;
use mcrs_minecraft_level::session::PlayerSession;

/// Host-side pending inbound buffer, keyed by host_anchor entity.
/// Holds packets received while the player's in-dim spawn is in flight
/// (in_dim_entity is None or dim is PLACEHOLDER). Drained by
/// `bridge_player_attach` once the sub-app signals the attach is complete.
#[derive(Resource, Default)]
pub struct PendingInboundBuffer {
    pub buffers: FxHashMap<Entity, SmallVec<[InboundPlayerPacket; 4]>>,
}

#[derive(Resource, Default)]
pub struct PlayerIndex {
    players: FxHashMap<String, PlayerSession>,
}

impl PlayerIndex {
    pub fn insert_username(&mut self, username: String, session: PlayerSession) {
        self.players.insert(username, session);
    }

    pub fn get_by_username(&self, username: &str) -> Option<PlayerSession> {
        self.players.get(username).copied()
    }

    /// Keyed by session rather than name: a player who logs in again before the
    /// old connection is cleaned up already owns the name.
    pub fn remove_session(&mut self, session: PlayerSession) {
        self.players.retain(|_, held| *held != session);
    }

    pub fn len(&self) -> usize {
        self.players.len()
    }

    pub fn is_empty(&self) -> bool {
        self.players.is_empty()
    }
}

#[derive(Component, Clone, Copy, Debug)]
pub struct PlayerSessionRef(pub PlayerSession);

#[derive(Component, Clone, Copy, Debug)]
pub struct HostAnchorRef(pub Entity);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_index_default_is_empty() {
        let index = PlayerIndex::default();
        assert!(index.is_empty());
        assert_eq!(index.len(), 0);
    }

    #[test]
    fn insert_username_then_get_by_username() {
        let mut index = PlayerIndex::default();
        let session = PlayerSession(1);
        index.insert_username("alice".into(), session);
        assert_eq!(index.get_by_username("alice"), Some(session));
        assert_eq!(index.get_by_username("bob"), None);
    }

    #[test]
    fn remove_session_drops_only_that_sessions_name() {
        let mut index = PlayerIndex::default();
        index.insert_username("bob".into(), PlayerSession(2));
        index.insert_username("alice".into(), PlayerSession(3));
        index.remove_session(PlayerSession(2));
        assert_eq!(index.get_by_username("bob"), None);
        assert_eq!(index.get_by_username("alice"), Some(PlayerSession(3)));
        index.remove_session(PlayerSession(2));
        assert_eq!(index.len(), 1);
    }

    #[test]
    fn len_and_is_empty_reflect_insertions() {
        let mut index = PlayerIndex::default();
        assert!(index.is_empty());
        index.insert_username("a".into(), PlayerSession(1));
        assert_eq!(index.len(), 1);
        assert!(!index.is_empty());
        index.insert_username("b".into(), PlayerSession(2));
        assert_eq!(index.len(), 2);
        index.remove_session(PlayerSession(1));
        assert_eq!(index.len(), 1);
    }
}

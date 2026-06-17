use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::resource::Resource;
use rustc_hash::FxHashMap;

/// Never-reused process-global routing key. Copy + Hash + Eq.
/// Not a Component — it is embedded in Owner and SessionEntry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PlayerSession(pub u64);

/// Marks a DimWorld player entity as owned by a session.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Owner(pub PlayerSession);

pub struct SessionEntry {
    /// The MainWorld connection (socket) entity for this session.
    pub connection_entity: Entity,
    /// The MainWorld host-anchor entity spawned at login.
    pub host_anchor: Entity,
    /// The current dim sub-app entity. `Entity::PLACEHOLDER` until assigned.
    pub dim: Entity,
    /// The previous dim (set during transfer, cleared after attach).
    pub previous_dim: Option<Entity>,
    /// The in-dim player entity once the spawn completes.
    pub in_dim_entity: Option<Entity>,
    pub epoch: u32,
}

#[derive(Resource, Default)]
pub struct SessionRegistry {
    entries: FxHashMap<PlayerSession, SessionEntry>,
    /// Reverse lookup: host_anchor entity → PlayerSession.
    by_anchor: FxHashMap<Entity, PlayerSession>,
}

impl SessionRegistry {
    pub fn insert(&mut self, session: PlayerSession, entry: SessionEntry) {
        self.by_anchor.insert(entry.host_anchor, session);
        self.entries.insert(session, entry);
    }

    pub fn get(&self, session: &PlayerSession) -> Option<&SessionEntry> {
        self.entries.get(session)
    }

    pub fn get_mut(&mut self, session: &PlayerSession) -> Option<&mut SessionEntry> {
        self.entries.get_mut(session)
    }

    pub fn get_by_anchor(&self, host_anchor: &Entity) -> Option<(&PlayerSession, &SessionEntry)> {
        let session = self.by_anchor.get(host_anchor)?;
        let entry = self.entries.get(session)?;
        Some((session, entry))
    }

    pub fn get_by_anchor_mut(
        &mut self,
        host_anchor: &Entity,
    ) -> Option<(PlayerSession, &mut SessionEntry)> {
        let session = *self.by_anchor.get(host_anchor)?;
        let entry = self.entries.get_mut(&session)?;
        Some((session, entry))
    }

    pub fn remove(&mut self, session: &PlayerSession) -> Option<SessionEntry> {
        if let Some(entry) = self.entries.get(session) {
            self.by_anchor.remove(&entry.host_anchor);
        }
        self.entries.remove(session)
    }

    pub fn contains(&self, session: &PlayerSession) -> bool {
        self.entries.contains_key(session)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&PlayerSession, &SessionEntry)> {
        self.entries.iter()
    }

    pub fn iter_in_dim(&self, dim: Entity) -> impl Iterator<Item = (&PlayerSession, &SessionEntry)> {
        self.entries.iter().filter(move |(_, e)| e.dim == dim)
    }
}

/// Counter that never emits PlayerSession(0). Starts at 1 on first call so
/// an unstamped packet with the default PlayerSession(0) never matches any
/// live session in SessionRegistry.
#[derive(Resource, Default)]
pub struct PlayerSessionCounter(u64);

impl PlayerSessionCounter {
    pub fn next(&mut self) -> PlayerSession {
        self.0 += 1;
        PlayerSession(self.0)
    }
}

/// Per-dim O(1) reverse map: session → dim-local entity.
/// Inserted fresh when a DimSubApp is created; never crosses a boundary.
#[derive(Resource, Default)]
pub struct DimPlayerIndex(pub FxHashMap<PlayerSession, Entity>);

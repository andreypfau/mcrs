use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::resource::Resource;
use rustc_hash::FxHashMap;

/// Never-reused process-global routing key. Copy + Hash + Eq.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PlayerSession(pub u64);

/// Named by the dimension that started the move, which alone counts its moves, so no two
/// dimensions can hand the host the same id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MoveId {
    pub source: Entity,
    pub seq: u64,
}

/// Marks a DimWorld player entity as owned by a session.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Owner(pub PlayerSession);

/// A logged-in player on the host, carried by its host anchor entity for as long as the
/// player is connected.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
#[require(SessionPlacement)]
pub struct Session(pub PlayerSession);

/// Where the host routes a session's packets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Place {
    #[default]
    Unplaced,
    /// Spawned into a dimension that has not yet attached the player.
    Joining(Entity),
    InDim(Entity),
    /// Moving between dimensions; the source keeps the player hidden until the move is
    /// confirmed or rolled back.
    Transferring {
        from: Entity,
        to: Entity,
    },
}

impl Place {
    /// The dimension the session's clientbound packets belong to.
    pub fn dim(self) -> Option<Entity> {
        match self {
            Place::Unplaced => None,
            Place::Joining(dim) | Place::InDim(dim) | Place::Transferring { to: dim, .. } => {
                Some(dim)
            }
        }
    }

    /// The dimension that accepts the session's serverbound packets.
    pub fn attached(self) -> Option<Entity> {
        match self {
            Place::InDim(dim) => Some(dim),
            _ => None,
        }
    }

    /// Every dimension that may hold an entity of the session.
    pub fn holding_dims(self) -> impl Iterator<Item = Entity> {
        let (dim, leaving) = match self {
            Place::Unplaced => (None, None),
            Place::Joining(dim) | Place::InDim(dim) => (Some(dim), None),
            Place::Transferring { from, to } => (Some(to), (from != to).then_some(from)),
        };
        dim.into_iter().chain(leaving)
    }
}

/// `epoch` counts the dimensions a session has left, so a packet stamped in one it has
/// since left is dropped at the bridge.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SessionPlacement {
    place: Place,
    epoch: u32,
}

impl SessionPlacement {
    pub fn new(place: Place, epoch: u32) -> Self {
        Self { place, epoch }
    }

    pub fn place(&self) -> Place {
        self.place
    }

    pub fn epoch(&self) -> u32 {
        self.epoch
    }

    pub fn set(&mut self, next: Place) {
        if let Some(current) = self.place.dim()
            && next.dim() != Some(current)
        {
            self.epoch = self.epoch.wrapping_add(1);
        }
        self.place = next;
    }
}

/// Counter that never emits PlayerSession(0), so an unstamped packet carrying the default
/// PlayerSession(0) never matches a live session.
#[derive(Resource, Default)]
pub struct PlayerSessionCounter(u64);

impl PlayerSessionCounter {
    pub fn next(&mut self) -> PlayerSession {
        self.0 = self
            .0
            .checked_add(1)
            .expect("PlayerSession counter exhausted");
        PlayerSession(self.0)
    }
}

/// Per-dim O(1) reverse map: session → dim-local entity.
/// Inserted fresh when a DimSubApp is created; never crosses a boundary.
#[derive(Resource, Default)]
pub struct DimPlayerIndex(pub FxHashMap<PlayerSession, Entity>);

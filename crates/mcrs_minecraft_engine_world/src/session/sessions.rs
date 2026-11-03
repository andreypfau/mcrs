use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::lifecycle::HookContext;
use bevy_ecs::resource::Resource;
use bevy_ecs::world::{DeferredWorld, World};
use rustc_hash::FxHashMap;

/// The name of one connection of a player, in every world and on both sides of the
/// host↔dimension channels. The host never hands out a value twice, so a packet for a session
/// that has ended reaches no later connection of the same profile. A change of dimension keeps
/// it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PlayerSession(pub u64);

/// Marks a DimWorld player entity as owned by a session, and enters it in the
/// [`DimPlayerIndex`] of its world.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[component(on_insert = Owner::index, on_discard = Owner::unindex)]
pub struct Owner(pub PlayerSession);

impl Owner {
    fn index(mut world: DeferredWorld, ctx: HookContext) {
        let Some(&Owner(session)) = world.get::<Owner>(ctx.entity) else {
            return;
        };
        let entity = ctx.entity;
        match world.get_resource_mut::<DimPlayerIndex>() {
            Some(mut index) => {
                index.0.insert(session, entity);
            }
            None => world.commands().queue(move |world: &mut World| {
                world
                    .get_resource_or_init::<DimPlayerIndex>()
                    .0
                    .insert(session, entity);
            }),
        }
    }

    fn unindex(mut world: DeferredWorld, ctx: HookContext) {
        let Some(&Owner(session)) = world.get::<Owner>(ctx.entity) else {
            return;
        };
        if let Some(mut index) = world.get_resource_mut::<DimPlayerIndex>()
            && index.0.get(&session) == Some(&ctx.entity)
        {
            index.0.remove(&session);
        }
    }
}

/// A logged-in player on the host, carried by its host anchor entity for as long as the
/// player is connected, and entered in the host's [`SessionAnchors`].
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
#[require(SessionPlacement)]
#[component(on_insert = Session::index, on_discard = Session::unindex)]
pub struct Session(pub PlayerSession);

impl Session {
    fn index(mut world: DeferredWorld, ctx: HookContext) {
        let Some(&Session(session)) = world.get::<Session>(ctx.entity) else {
            return;
        };
        let entity = ctx.entity;
        match world.get_resource_mut::<SessionAnchors>() {
            Some(mut anchors) => {
                anchors.0.insert(session, entity);
            }
            None => world.commands().queue(move |world: &mut World| {
                world
                    .get_resource_or_init::<SessionAnchors>()
                    .0
                    .insert(session, entity);
            }),
        }
    }

    fn unindex(mut world: DeferredWorld, ctx: HookContext) {
        let Some(&Session(session)) = world.get::<Session>(ctx.entity) else {
            return;
        };
        if let Some(mut anchors) = world.get_resource_mut::<SessionAnchors>()
            && anchors.0.get(&session) == Some(&ctx.entity)
        {
            anchors.0.remove(&session);
        }
    }
}

/// Host-world index: session → the host anchor that carries its [`Session`]. The host resolves
/// every session that a dimension names through it; it never crosses a boundary.
#[derive(Resource, Default, Debug)]
pub struct SessionAnchors(FxHashMap<PlayerSession, Entity>);

impl SessionAnchors {
    pub fn get(&self, session: PlayerSession) -> Option<Entity> {
        self.0.get(&session).copied()
    }
}

/// Where the host routes a session's packets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Place {
    #[default]
    Unplaced,
    /// Spawned into a dimension that has not yet attached the player.
    Joining(Entity),
    InDim(Entity),
}

impl Place {
    /// The dimension the session's clientbound packets belong to.
    pub fn dim(self) -> Option<Entity> {
        match self {
            Place::Unplaced => None,
            Place::Joining(dim) | Place::InDim(dim) => Some(dim),
        }
    }

    /// The dimension that accepts the session's serverbound packets.
    pub fn attached(self) -> Option<Entity> {
        match self {
            Place::InDim(dim) => Some(dim),
            _ => None,
        }
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
    #[expect(
        clippy::should_implement_trait,
        reason = "the counter is a resource that hands out sessions, not an iterator, and the server calls this public method by name"
    )]
    pub fn next(&mut self) -> PlayerSession {
        self.0 = self.0.checked_add(1).unwrap_or(1);
        PlayerSession(self.0)
    }
}

/// Per-dim O(1) reverse map: session → dim-local entity, kept by the hooks of [`Owner`].
/// Inserted fresh when a DimSubApp is created; never crosses a boundary.
#[derive(Resource, Default)]
pub struct DimPlayerIndex(pub FxHashMap<PlayerSession, Entity>);

#[cfg(test)]
mod tests {
    use super::*;

    fn dim(index: u32) -> Entity {
        Entity::from_raw_u32(index).unwrap()
    }

    #[test]
    fn counter_starts_at_one_and_never_repeats() {
        let mut counter = PlayerSessionCounter::default();
        let mut sessions = Vec::with_capacity(1000);
        for _ in 0..1000 {
            sessions.push(counter.next());
        }
        assert_eq!(
            sessions[0],
            PlayerSession(1),
            "first allocation must be PlayerSession(1)"
        );
        for i in 1..sessions.len() {
            assert!(
                sessions[i].0 > sessions[i - 1].0,
                "session ids must be strictly increasing: {:?} <= {:?}",
                sessions[i],
                sessions[i - 1],
            );
        }
        assert!(
            sessions.iter().all(|s| s.0 != 0),
            "PlayerSession(0) must never be produced"
        );
    }

    #[test]
    fn the_epoch_advances_only_when_a_session_leaves_a_dimension() {
        let mut placement = SessionPlacement::default();
        placement.set(Place::Joining(dim(1)));
        placement.set(Place::InDim(dim(1)));
        assert_eq!(placement.place(), Place::InDim(dim(1)));
        assert_eq!(placement.epoch(), 0, "joining a first dimension");

        let mut placement = SessionPlacement::new(Place::InDim(dim(1)), 3);
        placement.set(Place::InDim(dim(1)));
        assert_eq!(placement.epoch(), 3, "staying in one dimension");
        placement.set(Place::InDim(dim(2)));
        assert_eq!(placement.epoch(), 4, "moving to another dimension");
        placement.set(Place::Unplaced);
        assert_eq!(placement.epoch(), 5, "leaving for reconfiguration");
    }

    #[test]
    fn only_an_attached_dimension_takes_serverbound_packets() {
        let a = dim(1);
        assert_eq!(Place::Unplaced.dim(), None);
        assert_eq!(Place::Joining(a).dim(), Some(a));
        assert_eq!(Place::Joining(a).attached(), None);
        assert_eq!(Place::InDim(a).attached(), Some(a));
    }
}

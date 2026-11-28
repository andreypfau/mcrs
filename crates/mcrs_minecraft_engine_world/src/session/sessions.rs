use bevy_ecs::component::{Component, Mutable};
use bevy_ecs::entity::Entity;
use bevy_ecs::lifecycle::HookContext;
use bevy_ecs::resource::Resource;
use bevy_ecs::world::{DeferredWorld, FromWorld, World};
use rustc_hash::FxHashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PlayerSession(pub u64);

impl PlayerSession {
    pub const SERVER: PlayerSession = PlayerSession(u64::MAX);
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[component(on_insert = Owner::index, on_discard = Owner::unindex)]
pub struct Owner(pub PlayerSession);

impl Owner {
    fn index(world: DeferredWorld, ctx: HookContext) {
        index_session(
            world,
            ctx,
            |&Owner(session)| session,
            |index: &mut DimPlayerIndex| &mut index.0,
        );
    }

    fn unindex(world: DeferredWorld, ctx: HookContext) {
        unindex_session(
            world,
            ctx,
            |&Owner(session)| session,
            |index: &mut DimPlayerIndex| &mut index.0,
        );
    }
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
#[require(SessionPlacement)]
#[component(on_insert = Session::index, on_discard = Session::unindex)]
pub struct Session(pub PlayerSession);

impl Session {
    fn index(world: DeferredWorld, ctx: HookContext) {
        index_session(
            world,
            ctx,
            |&Session(session)| session,
            |anchors: &mut SessionAnchors| &mut anchors.0,
        );
    }

    fn unindex(world: DeferredWorld, ctx: HookContext) {
        unindex_session(
            world,
            ctx,
            |&Session(session)| session,
            |anchors: &mut SessionAnchors| &mut anchors.0,
        );
    }
}

fn index_session<C: Component, R: Resource<Mutability = Mutable> + FromWorld>(
    mut world: DeferredWorld,
    ctx: HookContext,
    session_of: fn(&C) -> PlayerSession,
    map: fn(&mut R) -> &mut FxHashMap<PlayerSession, Entity>,
) {
    let Some(session) = world.get::<C>(ctx.entity).map(session_of) else {
        return;
    };
    let entity = ctx.entity;
    match world.get_resource_mut::<R>() {
        Some(mut resource) => {
            map(&mut resource).insert(session, entity);
        }
        None => world.commands().queue(move |world: &mut World| {
            map(&mut world.get_resource_or_init::<R>()).insert(session, entity);
        }),
    }
}

fn unindex_session<C: Component, R: Resource<Mutability = Mutable>>(
    mut world: DeferredWorld,
    ctx: HookContext,
    session_of: fn(&C) -> PlayerSession,
    map: fn(&mut R) -> &mut FxHashMap<PlayerSession, Entity>,
) {
    let Some(session) = world.get::<C>(ctx.entity).map(session_of) else {
        return;
    };
    if let Some(mut resource) = world.get_resource_mut::<R>() {
        let entries = map(&mut resource);
        if entries.get(&session) == Some(&ctx.entity) {
            entries.remove(&session);
        }
    }
}

#[derive(Resource, Default, Debug)]
pub struct SessionAnchors(FxHashMap<PlayerSession, Entity>);

impl SessionAnchors {
    pub fn get(&self, session: PlayerSession) -> Option<Entity> {
        self.0.get(&session).copied()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Place {
    #[default]
    Unplaced,
    Joining(Entity),
    InDim(Entity),
}

impl Place {
    pub fn dim(self) -> Option<Entity> {
        match self {
            Place::Unplaced => None,
            Place::Joining(dim) | Place::InDim(dim) => Some(dim),
        }
    }

    pub fn attached(self) -> Option<Entity> {
        match self {
            Place::InDim(dim) => Some(dim),
            _ => None,
        }
    }
}

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

#[derive(Resource, Default)]
pub struct PlayerSessionCounter(u64);

impl PlayerSessionCounter {
    #[expect(
        clippy::should_implement_trait,
        reason = "the counter is a resource and the server calls this by name"
    )]
    pub fn next(&mut self) -> PlayerSession {
        self.0 = self.0.checked_add(1).unwrap_or(1);
        PlayerSession(self.0)
    }
}

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

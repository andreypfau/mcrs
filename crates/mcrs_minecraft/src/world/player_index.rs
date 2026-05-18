use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::resource::Resource;
use rustc_hash::FxHashMap;
use smallvec::SmallVec;

use crate::world::bus::InboundPlayerPacket;

#[derive(Resource, Default)]
pub struct PlayerIndex {
    players: FxHashMap<Entity, PlayerLocation>,
}

impl PlayerIndex {
    pub fn get(&self, entity: &Entity) -> Option<&PlayerLocation> {
        self.players.get(entity)
    }

    pub fn get_mut(&mut self, entity: &Entity) -> Option<&mut PlayerLocation> {
        self.players.get_mut(entity)
    }

    pub fn insert(&mut self, entity: Entity, location: PlayerLocation) {
        self.players.insert(entity, location);
    }

    pub fn remove(&mut self, entity: &Entity) -> Option<PlayerLocation> {
        self.players.remove(entity)
    }

    pub fn contains(&self, entity: &Entity) -> bool {
        self.players.contains_key(entity)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Entity, &PlayerLocation)> {
        self.players.iter()
    }

    pub fn len(&self) -> usize {
        self.players.len()
    }

    pub fn is_empty(&self) -> bool {
        self.players.is_empty()
    }
}

pub struct PlayerLocation {
    pub socket: Entity,
    pub current_dim: Entity,
    pub in_dim_entity: Option<Entity>,
    pub inbound_pending: SmallVec<[InboundPlayerPacket; 4]>,
}

#[derive(Component, Clone, Copy, Debug)]
pub struct HostAnchorRef(pub Entity);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::bus::TestInboundPayload;

    fn placeholder_entity() -> Entity {
        Entity::PLACEHOLDER
    }

    fn make_location() -> PlayerLocation {
        PlayerLocation {
            socket: placeholder_entity(),
            current_dim: placeholder_entity(),
            in_dim_entity: None,
            inbound_pending: SmallVec::new(),
        }
    }

    #[test]
    fn player_index_default_is_empty() {
        let index = PlayerIndex::default();
        assert!(index.is_empty());
        assert_eq!(index.len(), 0);
    }

    #[test]
    fn insert_then_get() {
        let mut index = PlayerIndex::default();
        let e = placeholder_entity();
        index.insert(e, make_location());
        let got = index.get(&e).expect("just inserted");
        assert_eq!(got.socket, placeholder_entity());
        assert_eq!(got.current_dim, placeholder_entity());
        assert!(got.in_dim_entity.is_none());
    }

    #[test]
    fn insert_then_get_mut_mutates() {
        let mut index = PlayerIndex::default();
        let e = placeholder_entity();
        index.insert(e, make_location());
        {
            let loc = index.get_mut(&e).expect("just inserted");
            loc.in_dim_entity = Some(placeholder_entity());
        }
        let got = index.get(&e).expect("still inserted");
        assert_eq!(got.in_dim_entity, Some(placeholder_entity()));
    }

    #[test]
    fn remove_returns_value_then_none() {
        let mut index = PlayerIndex::default();
        let e = placeholder_entity();
        index.insert(e, make_location());
        assert!(index.remove(&e).is_some());
        assert!(index.remove(&e).is_none());
    }

    #[test]
    fn contains_after_insert_true_after_remove_false() {
        let mut index = PlayerIndex::default();
        let e = placeholder_entity();
        assert!(!index.contains(&e));
        index.insert(e, make_location());
        assert!(index.contains(&e));
        index.remove(&e);
        assert!(!index.contains(&e));
    }

    #[test]
    fn iter_yields_all_entries() {
        let mut index = PlayerIndex::default();
        let entity_a = Entity::from_raw_u32(1).expect("nonzero");
        let entity_b = Entity::from_raw_u32(2).expect("nonzero");
        index.insert(entity_a, make_location());
        index.insert(entity_b, make_location());

        let keys: Vec<Entity> = index.iter().map(|(e, _)| *e).collect();
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&entity_a));
        assert!(keys.contains(&entity_b));
    }

    #[test]
    fn state_machine_transit_then_attach() {
        let mut index = PlayerIndex::default();
        let host_anchor = placeholder_entity();
        index.insert(host_anchor, make_location());

        {
            let loc = index.get_mut(&host_anchor).expect("inserted");
            for seq in 0..3 {
                loc.inbound_pending.push(InboundPlayerPacket {
                    player: host_anchor,
                    packet: TestInboundPayload { seq },
                });
            }
        }

        {
            let loc = index.get(&host_anchor).expect("inserted");
            assert_eq!(loc.inbound_pending.len(), 3);
            assert!(loc.in_dim_entity.is_none());
        }

        let drained: Vec<InboundPlayerPacket> = {
            let loc = index.get_mut(&host_anchor).expect("inserted");
            loc.in_dim_entity = Some(host_anchor);
            loc.inbound_pending.drain(..).collect()
        };

        assert_eq!(drained.len(), 3);
        let loc = index.get(&host_anchor).expect("inserted");
        assert!(loc.inbound_pending.is_empty());
        assert_eq!(loc.in_dim_entity, Some(host_anchor));
    }
}

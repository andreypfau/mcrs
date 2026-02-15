use bevy_ecs::prelude::Component;
use rustc_hash::{FxBuildHasher, FxHashMap};

#[derive(Default, Clone, Debug, Component)]
pub struct Enchantments {
    map: FxHashMap<u16, u8>,
}

impl Enchantments {
    pub const fn empty() -> Self {
        Self {
            map: FxHashMap::with_hasher(FxBuildHasher),
        }
    }

    pub fn get_level_by_id(&self, id: u16) -> u8 {
        self.map.get(&id).copied().unwrap_or(0)
    }

    pub fn has_enchantment(&self, id: u16) -> bool {
        self.map.contains_key(&id)
    }
}

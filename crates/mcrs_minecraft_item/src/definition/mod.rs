pub mod schema;

use std::sync::Arc;

use crate::{ComponentMap, Template};
#[cfg(feature = "bevy")]
use bevy_ecs::resource::Resource;
use mcrs_minecraft_core::ResourceLocation;
#[cfg(feature = "bevy")]
use mcrs_minecraft_registry::TagSource;
use mcrs_minecraft_registry::{BlockStateId, ItemId};
use rustc_hash::FxHashMap;

pub const CORPUS_DIRECTORY: &str = "mcrs/item_definition";
pub const FORMAT_VERSION: &str = "1.21.130";

#[derive(Debug)]
pub struct ItemEntry {
    pub identifier: ResourceLocation<Arc<str>>,
    pub id: ItemId,
    pub prototype: ComponentMap,
    pub block_placer: Option<BlockStateId>,
    pub container_slots: Option<u8>,
    pub crafting_remainder: Option<Template>,
}

#[derive(Debug, Default)]
pub struct ItemDefinitions {
    entries: Vec<ItemEntry>,
    by_identifier: FxHashMap<ResourceLocation<Arc<str>>, ItemId>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("`{identifier}` is defined at positions {first} and {second}")]
pub struct DuplicateItem {
    pub identifier: ResourceLocation<Arc<str>>,
    pub first: usize,
    pub second: usize,
}

impl ItemDefinitions {
    pub fn from_entries(mut entries: Vec<ItemEntry>) -> Result<Self, DuplicateItem> {
        let mut by_identifier =
            FxHashMap::with_capacity_and_hasher(entries.len(), Default::default());
        for (position, entry) in entries.iter_mut().enumerate() {
            let id = ItemId(u16::try_from(position).expect("item ids are 16 bits wide"));
            entry.id = id;
            if let Some(first) = by_identifier.insert(entry.identifier.clone(), id) {
                return Err(DuplicateItem {
                    identifier: entry.identifier.clone(),
                    first: usize::from(first.0),
                    second: position,
                });
            }
        }
        Ok(Self {
            entries,
            by_identifier,
        })
    }

    pub fn get(&self, id: ItemId) -> Option<&ItemEntry> {
        self.entries.get(id.0 as usize)
    }

    pub fn id_of(&self, location: &str) -> Option<ItemId> {
        self.by_identifier.get(location).copied()
    }

    pub fn iter(&self) -> impl Iterator<Item = &ItemEntry> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(feature = "bevy")]
#[derive(Resource, Clone, Debug)]
pub struct Items(pub Arc<ItemDefinitions>);

#[cfg(feature = "bevy")]
impl std::ops::Deref for Items {
    type Target = ItemDefinitions;

    fn deref(&self) -> &ItemDefinitions {
        &self.0
    }
}

#[cfg(feature = "bevy")]
impl TagSource for Items {
    type Id = u32;

    fn id_of(&self, loc: &str) -> Option<u32> {
        ItemDefinitions::id_of(self, loc).map(|id| u32::from(id.0))
    }

    fn capacity(&self) -> u32 {
        self.len() as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ComponentMap;

    fn entry(name: &str) -> ItemEntry {
        ItemEntry {
            identifier: ResourceLocation::minecraft(name),
            id: ItemId(u16::MAX),
            prototype: ComponentMap::default(),
            block_placer: None,
            container_slots: None,
            crafting_remainder: None,
        }
    }

    #[test]
    fn ids_follow_the_order_of_the_entries() {
        let table =
            ItemDefinitions::from_entries(vec![entry("stone"), entry("apple"), entry("dirt")])
                .unwrap();
        assert_eq!(table.id_of("minecraft:stone"), Some(ItemId(0)));
        assert_eq!(table.id_of("minecraft:apple"), Some(ItemId(1)));
        assert_eq!(table.id_of("minecraft:dirt"), Some(ItemId(2)));
        let ids: Vec<_> = table.iter().map(|entry| entry.id).collect();
        assert_eq!(ids, [ItemId(0), ItemId(1), ItemId(2)]);
        assert_eq!(
            table.get(ItemId(1)).unwrap().identifier.as_str(),
            "minecraft:apple"
        );
    }

    #[test]
    fn a_repeated_identifier_does_not_build() {
        let result = ItemDefinitions::from_entries(vec![entry("stone"), entry("stone")]);
        assert_eq!(
            result.err(),
            Some(DuplicateItem {
                identifier: ResourceLocation::minecraft("stone"),
                first: 0,
                second: 1,
            })
        );
    }

    #[test]
    fn no_entries_build_an_empty_table() {
        let table = ItemDefinitions::from_entries(Vec::new()).unwrap();
        assert_eq!(table.len(), 0);
        assert_eq!(table.id_of("minecraft:stone"), None);
    }
}

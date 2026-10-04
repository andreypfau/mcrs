pub mod schema;

use std::sync::Arc;

use crate::{ComponentMap, Item, Template};
#[cfg(feature = "bevy")]
use bevy_ecs::resource::Resource;
use mcrs_minecraft_core::ResourceLocation;
#[cfg(feature = "bevy")]
use mcrs_minecraft_registry::TagSource;
use mcrs_minecraft_registry::{BlockStateId, ItemId, NarrowError, Registry, UnknownEntry};

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

#[derive(Debug)]
pub struct ItemDefinitions {
    entries: Vec<ItemEntry>,
    registry: Registry<Item>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("`{identifier}` is defined at positions {first} and {second}")]
pub struct DuplicateItem {
    pub identifier: ResourceLocation<Arc<str>>,
    pub first: usize,
    pub second: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ItemTableError {
    #[error(transparent)]
    Unknown(#[from] UnknownEntry),
    #[error(transparent)]
    Duplicate(#[from] DuplicateItem),
    #[error("item `{identifier}`: {source}")]
    Narrow {
        identifier: ResourceLocation<Arc<str>>,
        source: NarrowError,
    },
    #[error("the registries report lists item `{identifier}` but no entry defines it")]
    Missing {
        identifier: ResourceLocation<Arc<str>>,
    },
}

impl Default for ItemDefinitions {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            registry: Registry::new(std::iter::empty(), std::iter::empty())
                .expect("a registry without entries builds"),
        }
    }
}

impl ItemDefinitions {
    pub fn from_entries(
        items: &Registry<Item>,
        entries: Vec<ItemEntry>,
    ) -> Result<Self, ItemTableError> {
        let mut placed: Vec<Option<(usize, ItemEntry)>> =
            std::iter::repeat_with(|| None).take(items.len()).collect();
        for (position, mut entry) in entries.into_iter().enumerate() {
            let id = items.require(entry.identifier.as_str())?;
            let narrowed: u16 = id.narrow().map_err(|source| ItemTableError::Narrow {
                identifier: entry.identifier.clone(),
                source,
            })?;
            entry.id = ItemId(narrowed);
            entry.identifier = items
                .key(id)
                .expect("the id came from this registry")
                .clone();
            if let Some((first, _)) = &placed[id.index()] {
                return Err(DuplicateItem {
                    identifier: entry.identifier,
                    first: *first,
                    second: position,
                }
                .into());
            }
            placed[id.index()] = Some((position, entry));
        }
        let entries = placed
            .into_iter()
            .zip(items.ids())
            .map(|(slot, id)| {
                slot.map(|(_, entry)| entry)
                    .ok_or_else(|| ItemTableError::Missing {
                        identifier: items
                            .key(id)
                            .expect("the id came from this registry")
                            .clone(),
                    })
            })
            .collect::<Result<_, _>>()?;
        Ok(Self {
            entries,
            registry: items.clone(),
        })
    }

    pub fn get(&self, id: ItemId) -> Option<&ItemEntry> {
        self.entries.get(usize::from(id.0))
    }

    pub fn id_of(&self, location: &str) -> Option<ItemId> {
        self.registry
            .get(location)?
            .narrow::<u16>()
            .ok()
            .map(ItemId)
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
impl mcrs_minecraft_registry::shared::SharedResource for Items {
    fn shares_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

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

    fn registry(names: &[&str]) -> Registry<Item> {
        Registry::new(
            names.iter().map(|name| ResourceLocation::minecraft(name)),
            std::iter::empty(),
        )
        .unwrap()
    }

    #[test]
    fn ids_follow_the_order_of_the_entries() {
        let table = ItemDefinitions::from_entries(
            &registry(&["stone", "apple", "dirt"]),
            vec![entry("stone"), entry("apple"), entry("dirt")],
        )
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
    fn entries_sit_at_their_report_id_whatever_their_order() {
        let table = ItemDefinitions::from_entries(
            &registry(&["stone", "apple", "dirt"]),
            vec![entry("dirt"), entry("stone"), entry("apple")],
        )
        .unwrap();
        let names: Vec<_> = table.iter().map(|e| e.identifier.as_str()).collect();
        assert_eq!(
            names,
            ["minecraft:stone", "minecraft:apple", "minecraft:dirt"]
        );
        let ids: Vec<_> = table.iter().map(|entry| entry.id).collect();
        assert_eq!(ids, [ItemId(0), ItemId(1), ItemId(2)]);
    }

    #[test]
    fn a_repeated_identifier_does_not_build() {
        let result = ItemDefinitions::from_entries(
            &registry(&["stone", "apple"]),
            vec![entry("stone"), entry("stone")],
        );
        assert_eq!(
            result.err(),
            Some(ItemTableError::Duplicate(DuplicateItem {
                identifier: ResourceLocation::minecraft("stone"),
                first: 0,
                second: 1,
            }))
        );
    }

    #[test]
    fn an_item_the_report_does_not_name_does_not_build() {
        let error =
            ItemDefinitions::from_entries(&registry(&["stone"]), vec![entry("apple")]).unwrap_err();
        assert!(matches!(error, ItemTableError::Unknown(_)), "{error}");
        assert!(error.to_string().contains("minecraft:apple"), "{error}");
    }

    #[test]
    fn a_report_item_without_an_entry_does_not_build() {
        let error =
            ItemDefinitions::from_entries(&registry(&["stone", "apple"]), vec![entry("stone")])
                .unwrap_err();
        assert_eq!(
            error,
            ItemTableError::Missing {
                identifier: ResourceLocation::minecraft("apple")
            }
        );
    }

    #[test]
    fn an_item_table_past_sixteen_bits_is_refused() {
        let names: Vec<String> = (0..=usize::from(u16::MAX) + 1)
            .map(|n| format!("n{n}"))
            .collect();
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        let last = names[names.len() - 1];
        let error =
            ItemDefinitions::from_entries(&registry(&names), vec![entry(last)]).unwrap_err();
        let ItemTableError::Narrow { identifier, source } = &error else {
            panic!("{error}");
        };
        assert_eq!(identifier.as_str(), format!("minecraft:{last}"));
        assert_eq!(source.id, u32::from(u16::MAX) + 1);
        assert_eq!(source.bits, 16);
        let message = error.to_string();
        assert!(message.contains("minecraft:item"), "{message}");
        assert!(message.contains("65536"), "{message}");
    }

    #[test]
    fn no_entries_build_an_empty_table() {
        let table = ItemDefinitions::from_entries(&registry(&[]), Vec::new()).unwrap();
        assert_eq!(table.len(), 0);
        assert_eq!(table.id_of("minecraft:stone"), None);
    }
}

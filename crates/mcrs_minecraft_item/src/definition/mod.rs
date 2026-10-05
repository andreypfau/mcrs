pub mod schema;

use std::sync::Arc;

use crate::{ComponentMap, Template};
#[cfg(feature = "bevy")]
use bevy_ecs::resource::Resource;
use mcrs_minecraft_core::{ResourceKey, ResourceLocation};
use mcrs_minecraft_keys::Item;
use mcrs_minecraft_registry::{BlockStateId, Id, Registry, UnknownEntry};

pub const CORPUS_DIRECTORY: &str = "mcrs/item_definition";
pub const FORMAT_VERSION: &str = "1.21.130";

#[derive(Debug)]
pub struct ItemEntry {
    pub identifier: ResourceLocation<Arc<str>>,
    pub id: Id<Item>,
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
    #[error("the registries report lists item `{identifier}` but no entry defines it")]
    Missing {
        identifier: ResourceLocation<Arc<str>>,
    },
}

impl Default for ItemDefinitions {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            registry: Registry::new(std::iter::empty()).expect("a registry without entries builds"),
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
            let id = items.require(&ResourceKey::from_location(entry.identifier.clone()))?;
            entry.id = id;
            entry.identifier = items
                .name(id)
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
                            .name(id)
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

    pub fn get(&self, id: Id<Item>) -> Option<&ItemEntry> {
        self.entries.get(id.index())
    }

    pub fn id_of(&self, location: &str) -> Option<Id<Item>> {
        self.registry.by_name(location)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ComponentMap;

    fn entry(name: &str) -> ItemEntry {
        ItemEntry {
            identifier: ResourceLocation::minecraft(name),
            id: Id::from_static(u16::MAX),
            prototype: ComponentMap::default(),
            block_placer: None,
            container_slots: None,
            crafting_remainder: None,
        }
    }

    fn registry(names: &[&str]) -> Registry<Item> {
        Registry::new(names.iter().map(|name| ResourceLocation::minecraft(name))).unwrap()
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
        let ids: Vec<_> = table.iter().map(|entry| entry.id.number()).collect();
        assert_eq!(ids, [0, 1, 2]);
        let id_of = |name| table.id_of(name).map(Id::number);
        assert_eq!(id_of("minecraft:stone"), Some(0));
        assert_eq!(id_of("minecraft:apple"), Some(1));
        assert_eq!(id_of("minecraft:dirt"), Some(2));
        let apple = table.id_of("minecraft:apple").unwrap();
        assert_eq!(
            table.get(apple).unwrap().identifier.as_str(),
            "minecraft:apple"
        );
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
    fn an_item_table_fills_all_sixteen_bits() {
        let names: Vec<String> = (0..=usize::from(u16::MAX))
            .map(|n| format!("n{n}"))
            .collect();
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        let last = names[names.len() - 1];
        let mut entries: Vec<_> = names.iter().map(|name| entry(name)).collect();
        entries.reverse();
        let table = ItemDefinitions::from_entries(&registry(&names), entries).unwrap();
        assert!(
            table
                .iter()
                .enumerate()
                .all(|(position, entry)| entry.id.index() == position)
        );
        let last_id = table.id_of(&format!("minecraft:{last}")).unwrap();
        assert_eq!(last_id.number(), u16::MAX);
        assert_eq!(
            table.get(last_id).unwrap().identifier.as_str(),
            format!("minecraft:{last}")
        );
    }

    #[test]
    fn no_entries_build_an_empty_table() {
        let table = ItemDefinitions::from_entries(&registry(&[]), Vec::new()).unwrap();
        assert_eq!(table.len(), 0);
        assert_eq!(table.id_of("minecraft:stone"), None);
    }
}

use alloc::borrow::ToOwned;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use mcrs_minecraft_item_stack::Builtin;
use mcrs_minecraft_item_stack::NameLookup;
use mcrs_minecraft_item_stack::NameTable;
use mcrs_minecraft_item_stack::SavedError;
use mcrs_minecraft_item_stack::read_saved_component;
use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_resource::ResourceLocationBuf;

use crate::BakeError;
use crate::DefinitionFile;
use crate::ItemTable;
use crate::SkippedType;
use crate::item_definition::DefinedItem;
use crate::item_definition::ItemDefinition;
use crate::item_names::ItemNames;

/// The table that the item definitions bake into, and the component types that it left out.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct BakedItems {
    pub table: ItemTable,
    /// The component types without a Rust type, in the order of their identifiers.
    pub skipped: Vec<SkippedType>,
}

/// Reads the item definitions and builds the table of their prototypes.
///
/// The item ids of the files must be dense from 0. The item names come from the files; `registries`
/// gives the ids of the other names that a component holds, and of each component type. A
/// component of a type without a Rust type is left out and counted. One bad file fails the bake.
pub fn bake<'file, L: NameLookup>(
    files: impl IntoIterator<Item = DefinitionFile<'file>>,
    registries: &L,
) -> Result<BakedItems, BakeError> {
    let definitions = read_items(files)?;
    let names: Vec<ResourceLocationBuf> = definitions
        .iter()
        .map(|definition| definition.description.identifier.clone())
        .collect();
    let by_name = ItemNames::sort(&names).map_err(|name| BakeError::RepeatedItem(name.clone()))?;
    let lookup = BakeLookup {
        items: ItemNames::new(&names, &by_name),
        registries,
    };
    let mut skipped: BTreeMap<ResourceLocationBuf, usize> = BTreeMap::new();
    let mut prototypes: Vec<Vec<Builtin>> = Vec::with_capacity(definitions.len());
    for definition in definitions {
        let mut components: Vec<Builtin> = Vec::with_capacity(definition.components.len());
        for (component, value) in &definition.components {
            let error = |cause: SavedError| BakeError::Component {
                item: definition.description.identifier.clone(),
                component: component.clone(),
                cause,
            };
            let location = component.as_location();
            match read_saved_component(location, &value.0, &lookup).map_err(error)? {
                Some(builtin) => components.push(builtin),
                None => {
                    if lookup.id(NameTable::ComponentType, location).is_none() {
                        return Err(error(SavedError::UnknownComponentType));
                    }
                    let count = skipped.entry(component.clone()).or_default();
                    *count = count.saturating_add(1);
                }
            }
        }
        prototypes.push(components);
    }
    Ok(BakedItems {
        table: ItemTable::leak_named(prototypes, names, by_name),
        skipped: skipped
            .into_iter()
            .map(|(component, prototypes)| SkippedType {
                component,
                prototypes,
            })
            .collect(),
    })
}

fn read_items<'file>(
    files: impl IntoIterator<Item = DefinitionFile<'file>>,
) -> Result<Vec<DefinedItem>, BakeError> {
    let mut by_id: Vec<Option<DefinedItem>> = Vec::new();
    for file in files {
        let definition: ItemDefinition =
            serde_json::from_slice(file.bytes).map_err(|error| BakeError::Layout {
                file: file.name.to_owned(),
                line: error.line(),
                column: error.column(),
            })?;
        let item = definition.item;
        let id = item.description.protocol_id;
        let index = usize::from(id);
        if by_id.len() <= index {
            by_id.resize_with(index.saturating_add(1), || None);
        }
        let Some(slot) = by_id.get_mut(index) else {
            return Err(BakeError::MissingId(id));
        };
        if slot.is_some() {
            return Err(BakeError::RepeatedId {
                item: item.description.identifier,
                id,
            });
        }
        *slot = Some(item);
    }
    let mut items: Vec<DefinedItem> = Vec::with_capacity(by_id.len());
    for (id, item) in (0..=u16::MAX).zip(by_id) {
        items.push(item.ok_or(BakeError::MissingId(id))?);
    }
    Ok(items)
}

struct BakeLookup<'bake, L> {
    items: ItemNames<'bake>,
    registries: &'bake L,
}

impl<L: NameLookup> NameLookup for BakeLookup<'_, L> {
    fn id(&self, table: NameTable, name: ResourceLocation<'_>) -> Option<u16> {
        match table {
            NameTable::Item => self.items.id(name),
            _ => self.registries.id(table, name),
        }
    }

    fn name(&self, table: NameTable, id: u16) -> Option<ResourceLocation<'_>> {
        match table {
            NameTable::Item => self.items.name(id),
            _ => self.registries.name(table, id),
        }
    }
}

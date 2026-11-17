use alloc::boxed::Box;
use alloc::vec::Vec;

use mcrs_minecraft_item_stack::Builtin;
use mcrs_minecraft_item_stack::ItemComponent;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_item_stack::NameLookup;
use mcrs_minecraft_item_stack::NameTable;
use mcrs_minecraft_item_stack::PatchEntry;
use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_resource::ResourceLocationBuf;

use crate::Prototype;
use crate::item_names::ItemNames;

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::resource::Resource))]
pub struct ItemTable {
    prototypes: &'static [Prototype],
    names: ItemNames<'static>,
}

impl ItemTable {
    #[must_use]
    pub fn leak(prototypes: impl IntoIterator<Item = Vec<Builtin>>) -> Self {
        Self {
            prototypes: leak_prototypes(prototypes),
            names: ItemNames::EMPTY,
        }
    }

    pub(crate) fn leak_named(
        prototypes: Vec<Vec<Builtin>>,
        names: Vec<ResourceLocationBuf>,
        by_name: Vec<u16>,
    ) -> Self {
        Self {
            prototypes: leak_prototypes(prototypes),
            names: ItemNames::new(
                Box::leak(names.into_boxed_slice()),
                Box::leak(by_name.into_boxed_slice()),
            ),
        }
    }

    #[must_use]
    pub fn prototype(&self, kind: Kind) -> Option<&'static Prototype> {
        self.prototypes.get(usize::from(kind.0))
    }

    #[must_use]
    pub fn effective<'stack, T: ItemComponent + 'static>(
        &self,
        stack: &'stack ItemStack,
    ) -> Option<&'stack T> {
        let entry = stack.patch().iter().find(|entry| T::is_entry_of(entry));
        match entry {
            Some(PatchEntry::Value(value)) => T::from_builtin(value),
            Some(PatchEntry::Undecoded(_) | PatchEntry::UndecodedWire(_)) => None,
            Some(
                PatchEntry::Removal(_) | PatchEntry::UndecodedRemoval(_) | PatchEntry::Custom(_),
            ) => None,
            None => self.prototype(stack.kind)?.get().or_else(T::neutral),
        }
    }
}

impl NameLookup for ItemTable {
    fn id(&self, table: NameTable, name: ResourceLocation<'_>) -> Option<u16> {
        match table {
            NameTable::Item => self.names.id(name),
            _ => None,
        }
    }

    fn name(&self, table: NameTable, id: u16) -> Option<ResourceLocation<'_>> {
        match table {
            NameTable::Item => self.names.name(id),
            _ => None,
        }
    }
}

fn leak_prototypes(prototypes: impl IntoIterator<Item = Vec<Builtin>>) -> &'static [Prototype] {
    let prototypes: Vec<Prototype> = prototypes
        .into_iter()
        .map(|components| Prototype::new(components.into_boxed_slice()))
        .collect();
    Box::leak(prototypes.into_boxed_slice())
}

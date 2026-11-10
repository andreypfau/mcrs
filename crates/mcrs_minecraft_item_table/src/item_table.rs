use alloc::boxed::Box;
use alloc::vec::Vec;

use mcrs_minecraft_item_stack::Builtin;
use mcrs_minecraft_item_stack::ItemComponent;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_item_stack::PatchEntry;

use crate::Prototype;

/// The prototype of each kind, indexed by the kind. Immutable once built.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::resource::Resource))]
pub struct ItemTable {
    prototypes: &'static [Prototype],
}

impl ItemTable {
    /// A table whose kind `n` has the components of the `n`-th input. The table is never freed,
    /// so each prototype value can be borrowed for `'static`.
    #[must_use]
    pub fn leak(prototypes: impl IntoIterator<Item = Vec<Builtin>>) -> Self {
        let prototypes: Vec<Prototype> = prototypes
            .into_iter()
            .map(|components| Prototype::new(components.into_boxed_slice()))
            .collect();
        // chisle: one leaked table for each data load; lifted by a table that the worlds of one
        // load borrow for a scoped lifetime, or by a generation counter that frees an unused table.
        Self {
            prototypes: Box::leak(prototypes.into_boxed_slice()),
        }
    }

    /// The prototype of `kind`, or `None` when the table has no such kind.
    #[must_use]
    pub fn prototype(&self, kind: Kind) -> Option<&'static Prototype> {
        self.prototypes.get(usize::from(kind.0))
    }

    /// The effective component of type `T` of `stack`: the value of its patch entry, none for a
    /// removal entry, else the component of the prototype. A custom type gives `None`, because
    /// the patch holds it encoded.
    #[must_use]
    pub fn effective<'stack, T: ItemComponent>(
        &self,
        stack: &'stack ItemStack,
    ) -> Option<&'stack T> {
        let entry = stack.patch().iter().find(|entry| T::is_entry_of(entry));
        match entry {
            Some(PatchEntry::Value(value)) => T::from_builtin(value),
            // chisle: a type that still travels as bytes has no typed effective value; lifted
            // when the codec of that type exists.
            Some(PatchEntry::Undecoded(_) | PatchEntry::UndecodedWire(_)) => None,
            Some(
                PatchEntry::Removal(_) | PatchEntry::UndecodedRemoval(_) | PatchEntry::Custom(_),
            ) => None,
            None => self.prototype(stack.kind)?.get(),
        }
    }
}

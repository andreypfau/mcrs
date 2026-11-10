use alloc::vec::Vec;
use core::hash::Hash;

use crate::AttributeModifiers;
use crate::Builtin;
use crate::BuiltinKey;
use crate::CustomComponent;
use crate::CustomName;
use crate::CustomValue;
use crate::Damage;
use crate::Enchantments;
use crate::Food;
use crate::ItemModel;
use crate::ItemName;
use crate::Lore;
use crate::MaxDamage;
use crate::MaxStackSize;
use crate::PatchEntry;
use crate::Rarity;
use crate::RepairCost;
use crate::Tool;
use crate::TooltipDisplay;
use crate::Unbreakable;
use crate::patch_key::PatchKey;

/// A component type of an item stack.
///
/// Each built-in type and each [`CustomComponent`] has this trait. No other type can have it:
/// the hidden methods name a type that is private to this crate.
pub trait ItemComponent: Clone + PartialEq + Hash {
    #[doc(hidden)]
    fn patch_key() -> PatchKey<'static>;

    #[doc(hidden)]
    fn into_entry(self) -> PatchEntry;

    #[doc(hidden)]
    fn removal() -> Option<PatchEntry>;
}

// A prototype has no custom component, so the removal of one leaves no entry in the patch.
impl<C: CustomComponent> ItemComponent for C {
    fn patch_key() -> PatchKey<'static> {
        PatchKey::Custom(C::KEY)
    }

    fn into_entry(self) -> PatchEntry {
        let mut bytes = Vec::new();
        self.encode(&mut bytes);
        PatchEntry::Custom(CustomValue {
            key: C::KEY,
            bytes: bytes.into_boxed_slice(),
        })
    }

    fn removal() -> Option<PatchEntry> {
        None
    }
}

macro_rules! builtin_item_component {
    ($($name:ident),* $(,)?) => {
        $(
            impl ItemComponent for $name {
                fn patch_key() -> PatchKey<'static> {
                    PatchKey::Builtin(BuiltinKey::$name)
                }

                fn into_entry(self) -> PatchEntry {
                    PatchEntry::Value(Builtin::$name(self))
                }

                fn removal() -> Option<PatchEntry> {
                    Some(PatchEntry::Removal(BuiltinKey::$name))
                }
            }
        )*
    };
}

builtin_item_component!(
    MaxStackSize,
    MaxDamage,
    Damage,
    Unbreakable,
    CustomName,
    ItemName,
    ItemModel,
    Lore,
    Rarity,
    Enchantments,
    AttributeModifiers,
    TooltipDisplay,
    RepairCost,
    Food,
    Tool,
);

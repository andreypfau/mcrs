use alloc::vec::Vec;
use core::hash::Hash;

use crate::Builtin;
use crate::BuiltinKey;
use crate::CustomComponent;
use crate::CustomValue;
use crate::PatchEntry;
use crate::entity_form::EntityForm;
use crate::patch_key::PatchKey;

pub trait ItemComponent: Clone + PartialEq + Hash {
    #[doc(hidden)]
    fn patch_key() -> PatchKey<'static>;

    #[doc(hidden)]
    fn into_entry(self) -> PatchEntry;

    #[doc(hidden)]
    fn builtin_key() -> Option<BuiltinKey>;

    #[doc(hidden)]
    fn from_builtin(value: &Builtin) -> Option<&Self>;

    #[doc(hidden)]
    fn neutral() -> Option<&'static Self>
    where
        Self: 'static;

    #[doc(hidden)]
    fn borrow_static(value: &'static Self) -> Self
    where
        Self: 'static;

    #[doc(hidden)]
    fn is_entry_of(entry: &PatchEntry) -> bool {
        entry.key() == Self::patch_key()
    }
}

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

    fn builtin_key() -> Option<BuiltinKey> {
        None
    }

    fn from_builtin(_: &Builtin) -> Option<&Self> {
        None
    }

    fn neutral() -> Option<&'static Self>
    where
        Self: 'static,
    {
        None
    }

    fn borrow_static(value: &'static Self) -> Self
    where
        Self: 'static,
    {
        value.clone()
    }
}

macro_rules! builtin_item_component {
    ($($name:ident => $path:literal),* $(,)?) => {
        $(
            impl ItemComponent for crate::$name {
                fn patch_key() -> PatchKey<'static> {
                    PatchKey::Builtin(BuiltinKey::$name)
                }

                fn into_entry(self) -> PatchEntry {
                    PatchEntry::Value(Builtin::$name(self))
                }

                fn builtin_key() -> Option<BuiltinKey> {
                    Some(BuiltinKey::$name)
                }

                fn from_builtin(value: &Builtin) -> Option<&Self> {
                    match value {
                        Builtin::$name(value) => Some(value),
                        _ => None,
                    }
                }

                fn neutral() -> Option<&'static Self> {
                    <Self as EntityForm>::NEUTRAL
                }

                fn borrow_static(value: &'static Self) -> Self {
                    EntityForm::borrow_static(value)
                }
            }
        )*
    };
}

crate::for_each_builtin!(builtin_item_component);

pub mod component;
#[doc(hidden)]
pub mod harness;
pub mod hash_ops;
pub mod kind;
pub mod patch;
pub mod stack;

pub use component::*;
pub use kind::{ItemComponentKind, ItemComponentValue, ItemDataComponent};
pub use patch::{ComponentMap, ComponentPatch};
pub use stack::{HashedPatchMap, ItemStackValue, ItemStackWithSlot, Template};

/// Text whose `show_item` hover carries an item stack template.
pub type Text = mcrs_minecraft_text::Text<Template>;

#[cfg(feature = "bevy")]
mod bevy {
    use bevy_ecs::component::{Component, Mutable, StorageType};

    use crate::component::*;

    macro_rules! impl_component {
        ($($id:literal $name:literal : $ty:ident [$($flag:ident),*]),* $(,)?) => {
            $(impl_component!(@one $ty);)*
        };
        (@one Profile) => {};
        (@one $ty:ident) => {
            impl Component for $ty {
                const STORAGE_TYPE: StorageType = StorageType::Table;
                type Mutability = Mutable;
            }
        };
    }

    crate::for_each_data_component!(impl_component);
}

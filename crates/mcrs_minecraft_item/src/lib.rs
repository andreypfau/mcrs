use mcrs_minecraft_core::tag_key::TaggedRegistry;

pub mod component;
pub mod definition;
#[cfg(feature = "bevy")]
pub mod dropped;
#[cfg(feature = "bevy")]
pub mod effective;
pub mod enchantment;
#[doc(hidden)]
pub mod harness;
pub mod hash_ops;
#[cfg(feature = "bevy")]
pub mod held;
#[cfg(feature = "bevy")]
pub mod inventory;
#[cfg(feature = "bevy")]
pub mod item_stack;
pub mod kind;
pub mod patch;
pub mod stack;
pub mod tags;
#[cfg(feature = "bevy")]
pub mod tool;
#[cfg(feature = "bevy")]
pub mod trim;
#[cfg(feature = "bevy")]
pub mod value;

pub use component::*;
#[cfg(feature = "bevy")]
pub use definition::Items;
pub use definition::{ItemDefinitions, ItemEntry};
#[cfg(feature = "bevy")]
pub use dropped::{DroppedItem, Thrower};
#[cfg(feature = "bevy")]
pub use effective::{
    children, component_value, damage_value, has_component, has_non_default, is_damageable,
    is_damaged, is_stackable, max_damage, max_stack_size,
};
#[cfg(feature = "bevy")]
pub use held::{Held, Holds, SlotTable};
#[cfg(feature = "bevy")]
pub use inventory::{SelectedHotbarSlot, slots};
#[cfg(feature = "bevy")]
pub use item_stack::{ItemStack, StackRevision};
pub use kind::{ItemComponentKind, ItemComponentValue, ItemDataComponent};
pub use patch::{ComponentMap, ComponentPatch};
pub use stack::{HashedPatchMap, ItemStackValue, ItemStackWithSlot, ProtoStack, Template};
#[cfg(feature = "bevy")]
pub use value::{StackError, item_of, same_item_same_components, stack_to_slot, stack_to_value};

/// Text whose `show_item` hover carries an item stack template.
pub type Text = mcrs_minecraft_text::Text<Template>;

pub enum Item {}

impl TaggedRegistry for Item {
    const REGISTRY_PATH: &'static str = "item";
}

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

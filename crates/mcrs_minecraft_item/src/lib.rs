use mcrs_minecraft_core::tag_key::TaggedRegistry;

pub mod definition;
pub mod dropped;
pub mod effective;
pub mod enchantment;
pub mod held;
pub mod inventory;
pub mod stack;
pub mod tags;
pub mod tool;
pub mod trim;
pub mod value;

pub use definition::{ItemDefinitions, ItemEntry, Items, load_item_definitions, test_corpus};
pub use dropped::{DroppedItem, Thrower, WireStack};
pub use effective::{
    children, component_value, damage_value, has_component, has_non_default, is_damageable,
    is_damaged, is_stackable, max_damage, max_stack_size,
};
pub use held::{Held, Holds, SlotTable};
pub use inventory::{SelectedHotbarSlot, slots};
pub use stack::{ItemStack, StackRevision};
pub use value::{StackError, item_of, same_item_same_components, stack_to_slot, stack_to_value};

pub enum Item {}

impl TaggedRegistry for Item {
    const REGISTRY_PATH: &'static str = "item";
}

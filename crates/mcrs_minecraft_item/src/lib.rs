use mcrs_minecraft_core::tag_key::TaggedRegistry;

pub mod definition;
pub mod dropped;
pub mod effective;
pub mod enchantment;
pub mod held;
pub mod inventory;
pub mod mutate;
pub mod patch;
pub mod stack;
pub mod sync;
pub mod tags;
pub mod tool;
pub mod trim;
pub mod value;

pub use definition::{ItemDefinitions, ItemEntry, Items, load_item_definitions};
pub use dropped::{DroppedItem, Thrower, WireStack};
pub use effective::{StackComponent, effective, is_damageable, is_stackable, max_stack_size};
pub use held::{Held, Holds, SlotTable};
pub use inventory::{SelectedHotbarSlot, slots};
pub use mutate::{MoveError, StackCommands};
pub use patch::Patch;
pub use stack::{ItemStack, StackRevision};
pub use sync::DirtyStacks;
pub use value::{StackError, same_item_same_components, stack_to_slot, stack_to_value};

pub enum Item {}

impl TaggedRegistry for Item {
    const REGISTRY_PATH: &'static str = "item";
}

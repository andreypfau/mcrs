pub mod component;
mod component_value_seed;
pub mod definition;
pub mod dialog;
pub mod enchantment;
#[rustfmt::skip]
pub mod consume_effect_type_keys;
#[rustfmt::skip]
pub mod data_component_predicate_type_keys;
#[rustfmt::skip]
pub mod dialog_action_type_keys;
#[rustfmt::skip]
pub mod dialog_body_type_keys;
#[rustfmt::skip]
pub mod dialog_keys;
#[rustfmt::skip]
pub mod dialog_tag_keys;
#[rustfmt::skip]
pub mod dialog_type_keys;
#[rustfmt::skip]
pub mod enchantment_keys;
#[rustfmt::skip]
pub mod enchantment_tag_keys;
#[rustfmt::skip]
pub mod input_control_type_keys;
#[rustfmt::skip]
pub mod instrument_keys;
#[rustfmt::skip]
pub mod instrument_tag_keys;
#[rustfmt::skip]
pub mod jukebox_song_keys;
#[rustfmt::skip]
pub mod painting_variant_keys;
#[rustfmt::skip]
pub mod painting_variant_tag_keys;
#[rustfmt::skip]
pub mod registry_keys;
#[rustfmt::skip]
pub mod trim_material_keys;
#[rustfmt::skip]
pub mod trim_pattern_keys;
#[doc(hidden)]
#[expect(
    clippy::panic,
    clippy::unwrap_used,
    reason = "test support reports a failure by panicking"
)]
pub mod harness;
pub mod hash_ops;
pub mod inventory;
mod item_hover_map;
pub mod kind;
pub mod patch;
pub mod recipe;
pub mod stack;
mod wire;

pub use component::*;
#[cfg(feature = "bevy")]
pub use definition::Items;
pub use definition::{ItemDefinitions, ItemEntry, ItemTableError};
pub use inventory::{LAST_HOTBAR_SLOT, slots};
pub use item_hover_map::ItemHoverMap;
pub use kind::{ItemComponentValue, ItemDataComponent};
pub use mcrs_minecraft_item_stack::data_component_type_keys;
pub use patch::{ComponentMap, ComponentPatch};
pub use stack::{HashedPatchMap, ItemStackValue, ItemStackWithSlot, ProtoStack, Template};

pub type Text<M = mcrs_minecraft_registries::Resolved> = mcrs_minecraft_text::Text<Template<M>>;

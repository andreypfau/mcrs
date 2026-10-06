// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

pub mod recipe_display;
pub mod slot_display;

pub use recipe_display::RecipeDisplayType;
pub use slot_display::SlotDisplayType;

use mcrs_minecraft_core::{RegistryKey, TypeBinding, rl};
use mcrs_minecraft_registry::Registered;

pub const RECIPE_DISPLAY: RegistryKey<crate::keys::RecipeDisplayType> = RegistryKey::new(rl!("minecraft:recipe_display"));
impl Registered for crate::keys::RecipeDisplayType {
    const REGISTRY: RegistryKey<Self> = RECIPE_DISPLAY;
}

pub const SLOT_DISPLAY: RegistryKey<crate::keys::SlotDisplayType> = RegistryKey::new(rl!("minecraft:slot_display"));
impl Registered for crate::keys::SlotDisplayType {
    const REGISTRY: RegistryKey<Self> = SLOT_DISPLAY;
}

pub fn bindings() -> [TypeBinding; 2] {
    [
        RECIPE_DISPLAY.binding(),
        SLOT_DISPLAY.binding(),
    ]
}

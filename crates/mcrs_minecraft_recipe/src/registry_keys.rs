// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{Registered, RegistryKey, rl};

pub const RECIPE_BOOK_CATEGORY: RegistryKey<crate::recipe_book_category_keys::RecipeBookCategory> =
    RegistryKey::new(rl!("minecraft:recipe_book_category"));
impl Registered for crate::recipe_book_category_keys::RecipeBookCategory {
    const REGISTRY: RegistryKey<Self> = RECIPE_BOOK_CATEGORY;
}

pub const RECIPE_DISPLAY: RegistryKey<crate::recipe_display_keys::RecipeDisplayType> =
    RegistryKey::new(rl!("minecraft:recipe_display"));
impl Registered for crate::recipe_display_keys::RecipeDisplayType {
    const REGISTRY: RegistryKey<Self> = RECIPE_DISPLAY;
}

pub const RECIPE_SERIALIZER: RegistryKey<crate::recipe_serializer_keys::RecipeSerializer> =
    RegistryKey::new(rl!("minecraft:recipe_serializer"));
impl Registered for crate::recipe_serializer_keys::RecipeSerializer {
    const REGISTRY: RegistryKey<Self> = RECIPE_SERIALIZER;
}

pub const SLOT_DISPLAY: RegistryKey<crate::slot_display_keys::SlotDisplayType> =
    RegistryKey::new(rl!("minecraft:slot_display"));
impl Registered for crate::slot_display_keys::SlotDisplayType {
    const REGISTRY: RegistryKey<Self> = SLOT_DISPLAY;
}

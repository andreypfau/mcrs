use mcrs_minecraft_resource::{Registered, RegistryKey, rl};

pub const RECIPE_BOOK_CATEGORY: RegistryKey<crate::recipe_book_category_keys::RecipeBookCategory> =
    <crate::recipe_book_category_keys::RecipeBookCategory as Registered>::REGISTRY;
impl Registered for crate::recipe_book_category_keys::RecipeBookCategory {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:recipe_book_category"));
}

pub const RECIPE_DISPLAY: RegistryKey<crate::recipe_display_keys::RecipeDisplayType> =
    <crate::recipe_display_keys::RecipeDisplayType as Registered>::REGISTRY;
impl Registered for crate::recipe_display_keys::RecipeDisplayType {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:recipe_display"));
}

pub const RECIPE_SERIALIZER: RegistryKey<crate::recipe_serializer_keys::RecipeSerializer> =
    <crate::recipe_serializer_keys::RecipeSerializer as Registered>::REGISTRY;
impl Registered for crate::recipe_serializer_keys::RecipeSerializer {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:recipe_serializer"));
}

pub const SLOT_DISPLAY: RegistryKey<crate::slot_display_keys::SlotDisplayType> =
    <crate::slot_display_keys::SlotDisplayType as Registered>::REGISTRY;
impl Registered for crate::slot_display_keys::SlotDisplayType {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:slot_display"));
}

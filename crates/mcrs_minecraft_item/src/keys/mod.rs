// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

pub mod banner_pattern;
pub mod banner_pattern_tags;
pub mod block_transformer;
pub mod consume_effect_type;
pub mod context_float_provider;
pub mod context_float_provider_type;
pub mod context_int_provider;
pub mod context_int_provider_type;
pub mod context_key_set;
pub mod damage_type;
pub mod damage_type_tags;
pub mod data_component_predicate_type;
pub mod data_component_type;
pub mod decorated_pot_pattern;
pub mod dialog;
pub mod dialog_action_type;
pub mod dialog_body_type;
pub mod dialog_tags;
pub mod dialog_type;
pub mod enchantment;
pub mod enchantment_level_based_value_type;
pub mod enchantment_tags;
pub mod input_control_type;
pub mod instrument;
pub mod instrument_tags;
pub mod item;
pub mod item_tags;
pub mod jukebox_song;
pub mod loot_table;
pub mod map_decoration_type;
pub mod menu;
pub mod mob_effect;
pub mod painting_variant;
pub mod painting_variant_tags;
pub mod potion;
pub mod potion_tags;
pub mod recipe_book_category;
pub mod recipe_serializer;
pub mod trim_material;
pub mod trim_pattern;

pub use consume_effect_type::ConsumeEffectType;
pub use context_float_provider_type::ContextFloatProviderType;
pub use context_int_provider_type::ContextIntProviderType;
pub use context_key_set::ContextKeySet;
pub use data_component_predicate_type::DataComponentPredicateType;
pub use data_component_type::DataComponentType;
pub use dialog_action_type::DialogActionType;
pub use dialog_body_type::DialogBodyType;
pub use dialog_type::DialogType;
pub use enchantment_level_based_value_type::EnchantmentLevelBasedValueType;
pub use input_control_type::InputControlType;
pub use item::Item;
pub use map_decoration_type::MapDecorationType;
pub use menu::MenuType;
pub use mob_effect::MobEffect;
pub use potion::Potion;
pub use recipe_book_category::RecipeBookCategory;
pub use recipe_serializer::RecipeSerializer;

use mcrs_minecraft_core::{RegistryKey, TypeBinding, rl};
use mcrs_minecraft_registry::Registered;

pub const BANNER_PATTERN: RegistryKey<crate::BannerPattern> = RegistryKey::new(rl!("minecraft:banner_pattern"));
impl Registered for crate::BannerPattern {
    const REGISTRY: RegistryKey<Self> = BANNER_PATTERN;
}

pub const BLOCK_TRANSFORMER: RegistryKey<crate::block_transformer::BlockTransformer> = RegistryKey::new(rl!("minecraft:block_transformer"));
impl Registered for crate::block_transformer::BlockTransformer {
    const REGISTRY: RegistryKey<Self> = BLOCK_TRANSFORMER;
}

pub const CONSUME_EFFECT_TYPE: RegistryKey<crate::keys::ConsumeEffectType> = RegistryKey::new(rl!("minecraft:consume_effect_type"));
impl Registered for crate::keys::ConsumeEffectType {
    const REGISTRY: RegistryKey<Self> = CONSUME_EFFECT_TYPE;
}

pub const CONTEXT_FLOAT_PROVIDER: RegistryKey<crate::loot::ContextFloatProvider> = RegistryKey::new(rl!("minecraft:context_float_provider"));
impl Registered for crate::loot::ContextFloatProvider {
    const REGISTRY: RegistryKey<Self> = CONTEXT_FLOAT_PROVIDER;
}

pub const CONTEXT_FLOAT_PROVIDER_TYPE: RegistryKey<crate::keys::ContextFloatProviderType> = RegistryKey::new(rl!("minecraft:context_float_provider_type"));
impl Registered for crate::keys::ContextFloatProviderType {
    const REGISTRY: RegistryKey<Self> = CONTEXT_FLOAT_PROVIDER_TYPE;
}

pub const CONTEXT_INT_PROVIDER: RegistryKey<crate::loot::ContextIntProvider> = RegistryKey::new(rl!("minecraft:context_int_provider"));
impl Registered for crate::loot::ContextIntProvider {
    const REGISTRY: RegistryKey<Self> = CONTEXT_INT_PROVIDER;
}

pub const CONTEXT_INT_PROVIDER_TYPE: RegistryKey<crate::keys::ContextIntProviderType> = RegistryKey::new(rl!("minecraft:context_int_provider_type"));
impl Registered for crate::keys::ContextIntProviderType {
    const REGISTRY: RegistryKey<Self> = CONTEXT_INT_PROVIDER_TYPE;
}

pub const CONTEXT_KEY_SET: RegistryKey<crate::keys::ContextKeySet> = RegistryKey::new(rl!("minecraft:context_key_set"));
impl Registered for crate::keys::ContextKeySet {
    const REGISTRY: RegistryKey<Self> = CONTEXT_KEY_SET;
}

pub const DAMAGE_TYPE: RegistryKey<crate::damage_type::DamageType> = RegistryKey::new(rl!("minecraft:damage_type"));
impl Registered for crate::damage_type::DamageType {
    const REGISTRY: RegistryKey<Self> = DAMAGE_TYPE;
}

pub const DATA_COMPONENT_PREDICATE_TYPE: RegistryKey<crate::keys::DataComponentPredicateType> = RegistryKey::new(rl!("minecraft:data_component_predicate_type"));
impl Registered for crate::keys::DataComponentPredicateType {
    const REGISTRY: RegistryKey<Self> = DATA_COMPONENT_PREDICATE_TYPE;
}

pub const DATA_COMPONENT_TYPE: RegistryKey<crate::keys::DataComponentType> = RegistryKey::new(rl!("minecraft:data_component_type"));
impl Registered for crate::keys::DataComponentType {
    const REGISTRY: RegistryKey<Self> = DATA_COMPONENT_TYPE;
}

pub const DECORATED_POT_PATTERN: RegistryKey<crate::decorated_pot_pattern::DecoratedPotPattern> = RegistryKey::new(rl!("minecraft:decorated_pot_pattern"));
impl Registered for crate::decorated_pot_pattern::DecoratedPotPattern {
    const REGISTRY: RegistryKey<Self> = DECORATED_POT_PATTERN;
}

pub const DIALOG: RegistryKey<crate::dialog::Dialog> = RegistryKey::new(rl!("minecraft:dialog"));
impl Registered for crate::dialog::Dialog {
    const REGISTRY: RegistryKey<Self> = DIALOG;
}

pub const DIALOG_ACTION_TYPE: RegistryKey<crate::keys::DialogActionType> = RegistryKey::new(rl!("minecraft:dialog_action_type"));
impl Registered for crate::keys::DialogActionType {
    const REGISTRY: RegistryKey<Self> = DIALOG_ACTION_TYPE;
}

pub const DIALOG_BODY_TYPE: RegistryKey<crate::keys::DialogBodyType> = RegistryKey::new(rl!("minecraft:dialog_body_type"));
impl Registered for crate::keys::DialogBodyType {
    const REGISTRY: RegistryKey<Self> = DIALOG_BODY_TYPE;
}

pub const DIALOG_TYPE: RegistryKey<crate::keys::DialogType> = RegistryKey::new(rl!("minecraft:dialog_type"));
impl Registered for crate::keys::DialogType {
    const REGISTRY: RegistryKey<Self> = DIALOG_TYPE;
}

pub const ENCHANTMENT: RegistryKey<crate::enchantment::EnchantmentData> = RegistryKey::new(rl!("minecraft:enchantment"));
impl Registered for crate::enchantment::EnchantmentData {
    const REGISTRY: RegistryKey<Self> = ENCHANTMENT;
}

pub const ENCHANTMENT_LEVEL_BASED_VALUE_TYPE: RegistryKey<crate::keys::EnchantmentLevelBasedValueType> = RegistryKey::new(rl!("minecraft:enchantment_level_based_value_type"));
impl Registered for crate::keys::EnchantmentLevelBasedValueType {
    const REGISTRY: RegistryKey<Self> = ENCHANTMENT_LEVEL_BASED_VALUE_TYPE;
}

pub const INPUT_CONTROL_TYPE: RegistryKey<crate::keys::InputControlType> = RegistryKey::new(rl!("minecraft:input_control_type"));
impl Registered for crate::keys::InputControlType {
    const REGISTRY: RegistryKey<Self> = INPUT_CONTROL_TYPE;
}

pub const INSTRUMENT: RegistryKey<crate::InstrumentValue> = RegistryKey::new(rl!("minecraft:instrument"));
impl Registered for crate::InstrumentValue {
    const REGISTRY: RegistryKey<Self> = INSTRUMENT;
}

pub const ITEM: RegistryKey<crate::keys::Item> = RegistryKey::new(rl!("minecraft:item"));
impl Registered for crate::keys::Item {
    const REGISTRY: RegistryKey<Self> = ITEM;
}

pub const JUKEBOX_SONG: RegistryKey<crate::JukeboxSong> = RegistryKey::new(rl!("minecraft:jukebox_song"));
impl Registered for crate::JukeboxSong {
    const REGISTRY: RegistryKey<Self> = JUKEBOX_SONG;
}

pub const LOOT_TABLE: RegistryKey<crate::loot::LootTable> = RegistryKey::new(rl!("minecraft:loot_table"));
impl Registered for crate::loot::LootTable {
    const REGISTRY: RegistryKey<Self> = LOOT_TABLE;
}

pub const MAP_DECORATION_TYPE: RegistryKey<crate::keys::MapDecorationType> = RegistryKey::new(rl!("minecraft:map_decoration_type"));
impl Registered for crate::keys::MapDecorationType {
    const REGISTRY: RegistryKey<Self> = MAP_DECORATION_TYPE;
}

pub const MENU: RegistryKey<crate::keys::MenuType> = RegistryKey::new(rl!("minecraft:menu"));
impl Registered for crate::keys::MenuType {
    const REGISTRY: RegistryKey<Self> = MENU;
}

pub const MOB_EFFECT: RegistryKey<crate::keys::MobEffect> = RegistryKey::new(rl!("minecraft:mob_effect"));
impl Registered for crate::keys::MobEffect {
    const REGISTRY: RegistryKey<Self> = MOB_EFFECT;
}

pub const PAINTING_VARIANT: RegistryKey<crate::PaintingVariantValue> = RegistryKey::new(rl!("minecraft:painting_variant"));
impl Registered for crate::PaintingVariantValue {
    const REGISTRY: RegistryKey<Self> = PAINTING_VARIANT;
}

pub const POTION: RegistryKey<crate::keys::Potion> = RegistryKey::new(rl!("minecraft:potion"));
impl Registered for crate::keys::Potion {
    const REGISTRY: RegistryKey<Self> = POTION;
}

pub const RECIPE: RegistryKey<crate::recipe::Recipe> = RegistryKey::new(rl!("minecraft:recipe"));
impl Registered for crate::recipe::Recipe {
    const REGISTRY: RegistryKey<Self> = RECIPE;
}

pub const RECIPE_BOOK_CATEGORY: RegistryKey<crate::keys::RecipeBookCategory> = RegistryKey::new(rl!("minecraft:recipe_book_category"));
impl Registered for crate::keys::RecipeBookCategory {
    const REGISTRY: RegistryKey<Self> = RECIPE_BOOK_CATEGORY;
}

pub const RECIPE_SERIALIZER: RegistryKey<crate::keys::RecipeSerializer> = RegistryKey::new(rl!("minecraft:recipe_serializer"));
impl Registered for crate::keys::RecipeSerializer {
    const REGISTRY: RegistryKey<Self> = RECIPE_SERIALIZER;
}

pub const TRIM_MATERIAL: RegistryKey<crate::TrimMaterial> = RegistryKey::new(rl!("minecraft:trim_material"));
impl Registered for crate::TrimMaterial {
    const REGISTRY: RegistryKey<Self> = TRIM_MATERIAL;
}

pub const TRIM_PATTERN: RegistryKey<crate::TrimPattern> = RegistryKey::new(rl!("minecraft:trim_pattern"));
impl Registered for crate::TrimPattern {
    const REGISTRY: RegistryKey<Self> = TRIM_PATTERN;
}

pub fn bindings() -> [TypeBinding; 33] {
    [
        BANNER_PATTERN.binding(),
        BLOCK_TRANSFORMER.binding(),
        CONSUME_EFFECT_TYPE.binding(),
        CONTEXT_FLOAT_PROVIDER.binding(),
        CONTEXT_FLOAT_PROVIDER_TYPE.binding(),
        CONTEXT_INT_PROVIDER.binding(),
        CONTEXT_INT_PROVIDER_TYPE.binding(),
        CONTEXT_KEY_SET.binding(),
        DAMAGE_TYPE.binding(),
        DATA_COMPONENT_PREDICATE_TYPE.binding(),
        DATA_COMPONENT_TYPE.binding(),
        DECORATED_POT_PATTERN.binding(),
        DIALOG.binding(),
        DIALOG_ACTION_TYPE.binding(),
        DIALOG_BODY_TYPE.binding(),
        DIALOG_TYPE.binding(),
        ENCHANTMENT.binding(),
        ENCHANTMENT_LEVEL_BASED_VALUE_TYPE.binding(),
        INPUT_CONTROL_TYPE.binding(),
        INSTRUMENT.binding(),
        ITEM.binding(),
        JUKEBOX_SONG.binding(),
        LOOT_TABLE.binding(),
        MAP_DECORATION_TYPE.binding(),
        MENU.binding(),
        MOB_EFFECT.binding(),
        PAINTING_VARIANT.binding(),
        POTION.binding(),
        RECIPE.binding(),
        RECIPE_BOOK_CATEGORY.binding(),
        RECIPE_SERIALIZER.binding(),
        TRIM_MATERIAL.binding(),
        TRIM_PATTERN.binding(),
    ]
}

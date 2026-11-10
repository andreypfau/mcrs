// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{Registered, RegistryKey, rl};

pub const BANNER_PATTERN: RegistryKey<crate::BannerPattern> =
    RegistryKey::new(rl!("minecraft:banner_pattern"));
impl Registered for crate::BannerPattern {
    const REGISTRY: RegistryKey<Self> = BANNER_PATTERN;
}

pub const BLOCK_TRANSFORMER: RegistryKey<crate::block_transformer::BlockTransformer> =
    RegistryKey::new(rl!("minecraft:block_transformer"));
impl Registered for crate::block_transformer::BlockTransformer {
    const REGISTRY: RegistryKey<Self> = BLOCK_TRANSFORMER;
}

pub const CONSUME_EFFECT_TYPE: RegistryKey<crate::consume_effect_type_keys::ConsumeEffectType> =
    RegistryKey::new(rl!("minecraft:consume_effect_type"));
impl Registered for crate::consume_effect_type_keys::ConsumeEffectType {
    const REGISTRY: RegistryKey<Self> = CONSUME_EFFECT_TYPE;
}

pub const DATA_COMPONENT_PREDICATE_TYPE: RegistryKey<
    crate::data_component_predicate_type_keys::DataComponentPredicateType,
> = RegistryKey::new(rl!("minecraft:data_component_predicate_type"));
impl Registered for crate::data_component_predicate_type_keys::DataComponentPredicateType {
    const REGISTRY: RegistryKey<Self> = DATA_COMPONENT_PREDICATE_TYPE;
}

pub const DATA_COMPONENT_TYPE: RegistryKey<crate::data_component_type_keys::DataComponentType> =
    RegistryKey::new(rl!("minecraft:data_component_type"));
impl Registered for crate::data_component_type_keys::DataComponentType {
    const REGISTRY: RegistryKey<Self> = DATA_COMPONENT_TYPE;
}

pub const DECORATED_POT_PATTERN: RegistryKey<crate::decorated_pot_pattern::DecoratedPotPattern> =
    RegistryKey::new(rl!("minecraft:decorated_pot_pattern"));
impl Registered for crate::decorated_pot_pattern::DecoratedPotPattern {
    const REGISTRY: RegistryKey<Self> = DECORATED_POT_PATTERN;
}

pub const DIALOG: RegistryKey<crate::dialog::Dialog> = RegistryKey::new(rl!("minecraft:dialog"));
impl Registered for crate::dialog::Dialog {
    const REGISTRY: RegistryKey<Self> = DIALOG;
}

pub const DIALOG_ACTION_TYPE: RegistryKey<crate::dialog_action_type_keys::DialogActionType> =
    RegistryKey::new(rl!("minecraft:dialog_action_type"));
impl Registered for crate::dialog_action_type_keys::DialogActionType {
    const REGISTRY: RegistryKey<Self> = DIALOG_ACTION_TYPE;
}

pub const DIALOG_BODY_TYPE: RegistryKey<crate::dialog_body_type_keys::DialogBodyType> =
    RegistryKey::new(rl!("minecraft:dialog_body_type"));
impl Registered for crate::dialog_body_type_keys::DialogBodyType {
    const REGISTRY: RegistryKey<Self> = DIALOG_BODY_TYPE;
}

pub const DIALOG_TYPE: RegistryKey<crate::dialog_type_keys::DialogType> =
    RegistryKey::new(rl!("minecraft:dialog_type"));
impl Registered for crate::dialog_type_keys::DialogType {
    const REGISTRY: RegistryKey<Self> = DIALOG_TYPE;
}

pub const ENCHANTMENT: RegistryKey<crate::enchantment::Enchantment> =
    RegistryKey::new(rl!("minecraft:enchantment"));
impl Registered for crate::enchantment::Enchantment {
    const REGISTRY: RegistryKey<Self> = ENCHANTMENT;
}

pub const INPUT_CONTROL_TYPE: RegistryKey<crate::input_control_type_keys::InputControlType> =
    RegistryKey::new(rl!("minecraft:input_control_type"));
impl Registered for crate::input_control_type_keys::InputControlType {
    const REGISTRY: RegistryKey<Self> = INPUT_CONTROL_TYPE;
}

pub const INSTRUMENT: RegistryKey<crate::InstrumentValue> =
    RegistryKey::new(rl!("minecraft:instrument"));
impl Registered for crate::InstrumentValue {
    const REGISTRY: RegistryKey<Self> = INSTRUMENT;
}

pub const ITEM: RegistryKey<crate::item_keys::Item> = RegistryKey::new(rl!("minecraft:item"));
impl Registered for crate::item_keys::Item {
    const REGISTRY: RegistryKey<Self> = ITEM;
}

pub const JUKEBOX_SONG: RegistryKey<crate::JukeboxSong> =
    RegistryKey::new(rl!("minecraft:jukebox_song"));
impl Registered for crate::JukeboxSong {
    const REGISTRY: RegistryKey<Self> = JUKEBOX_SONG;
}

pub const MAP_DECORATION_TYPE: RegistryKey<crate::map_decoration_type_keys::MapDecorationType> =
    RegistryKey::new(rl!("minecraft:map_decoration_type"));
impl Registered for crate::map_decoration_type_keys::MapDecorationType {
    const REGISTRY: RegistryKey<Self> = MAP_DECORATION_TYPE;
}

pub const MENU: RegistryKey<crate::menu_keys::MenuType> = RegistryKey::new(rl!("minecraft:menu"));
impl Registered for crate::menu_keys::MenuType {
    const REGISTRY: RegistryKey<Self> = MENU;
}

pub const PAINTING_VARIANT: RegistryKey<crate::PaintingVariantValue> =
    RegistryKey::new(rl!("minecraft:painting_variant"));
impl Registered for crate::PaintingVariantValue {
    const REGISTRY: RegistryKey<Self> = PAINTING_VARIANT;
}

pub const POTION: RegistryKey<crate::potion_keys::Potion> =
    RegistryKey::new(rl!("minecraft:potion"));
impl Registered for crate::potion_keys::Potion {
    const REGISTRY: RegistryKey<Self> = POTION;
}

pub const RECIPE: RegistryKey<crate::recipe::Recipe> = RegistryKey::new(rl!("minecraft:recipe"));
impl Registered for crate::recipe::Recipe {
    const REGISTRY: RegistryKey<Self> = RECIPE;
}

pub const TRIM_MATERIAL: RegistryKey<crate::TrimMaterial> =
    RegistryKey::new(rl!("minecraft:trim_material"));
impl Registered for crate::TrimMaterial {
    const REGISTRY: RegistryKey<Self> = TRIM_MATERIAL;
}

pub const TRIM_PATTERN: RegistryKey<crate::TrimPattern> =
    RegistryKey::new(rl!("minecraft:trim_pattern"));
impl Registered for crate::TrimPattern {
    const REGISTRY: RegistryKey<Self> = TRIM_PATTERN;
}

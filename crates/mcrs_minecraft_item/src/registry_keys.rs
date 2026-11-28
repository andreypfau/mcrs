use mcrs_minecraft_resource::{Registered, RegistryKey, rl};

pub const CONSUME_EFFECT_TYPE: RegistryKey<crate::consume_effect_type_keys::ConsumeEffectType> =
    <crate::consume_effect_type_keys::ConsumeEffectType as Registered>::REGISTRY;
impl Registered for crate::consume_effect_type_keys::ConsumeEffectType {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:consume_effect_type"));
}

pub const DATA_COMPONENT_PREDICATE_TYPE: RegistryKey<
    crate::data_component_predicate_type_keys::DataComponentPredicateType,
> = <crate::data_component_predicate_type_keys::DataComponentPredicateType as Registered>::REGISTRY;
impl Registered for crate::data_component_predicate_type_keys::DataComponentPredicateType {
    const REGISTRY: RegistryKey<Self> =
        RegistryKey::new(rl!("minecraft:data_component_predicate_type"));
}

pub const DIALOG: RegistryKey<crate::dialog::Dialog> =
    <crate::dialog::Dialog as Registered>::REGISTRY;
impl Registered for crate::dialog::Dialog {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:dialog"));
}

pub const DIALOG_ACTION_TYPE: RegistryKey<crate::dialog_action_type_keys::DialogActionType> =
    <crate::dialog_action_type_keys::DialogActionType as Registered>::REGISTRY;
impl Registered for crate::dialog_action_type_keys::DialogActionType {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:dialog_action_type"));
}

pub const DIALOG_BODY_TYPE: RegistryKey<crate::dialog_body_type_keys::DialogBodyType> =
    <crate::dialog_body_type_keys::DialogBodyType as Registered>::REGISTRY;
impl Registered for crate::dialog_body_type_keys::DialogBodyType {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:dialog_body_type"));
}

pub const DIALOG_TYPE: RegistryKey<crate::dialog_type_keys::DialogType> =
    <crate::dialog_type_keys::DialogType as Registered>::REGISTRY;
impl Registered for crate::dialog_type_keys::DialogType {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:dialog_type"));
}

pub const ENCHANTMENT: RegistryKey<crate::enchantment::Enchantment> =
    <crate::enchantment::Enchantment as Registered>::REGISTRY;
impl Registered for crate::enchantment::Enchantment {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:enchantment"));
}

pub const INPUT_CONTROL_TYPE: RegistryKey<crate::input_control_type_keys::InputControlType> =
    <crate::input_control_type_keys::InputControlType as Registered>::REGISTRY;
impl Registered for crate::input_control_type_keys::InputControlType {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:input_control_type"));
}

pub const INSTRUMENT: RegistryKey<crate::InstrumentValue> =
    <crate::InstrumentValue as Registered>::REGISTRY;
impl Registered for crate::InstrumentValue {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:instrument"));
}

pub const JUKEBOX_SONG: RegistryKey<crate::JukeboxSong> =
    <crate::JukeboxSong as Registered>::REGISTRY;
impl Registered for crate::JukeboxSong {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:jukebox_song"));
}

pub const PAINTING_VARIANT: RegistryKey<crate::PaintingVariantValue> =
    <crate::PaintingVariantValue as Registered>::REGISTRY;
impl Registered for crate::PaintingVariantValue {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:painting_variant"));
}

pub const RECIPE: RegistryKey<crate::recipe::Recipe> =
    <crate::recipe::Recipe as Registered>::REGISTRY;
impl Registered for crate::recipe::Recipe {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:recipe"));
}

pub const TRIM_MATERIAL: RegistryKey<crate::TrimMaterial> =
    <crate::TrimMaterial as Registered>::REGISTRY;
impl Registered for crate::TrimMaterial {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:trim_material"));
}

pub const TRIM_PATTERN: RegistryKey<crate::TrimPattern> =
    <crate::TrimPattern as Registered>::REGISTRY;
impl Registered for crate::TrimPattern {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:trim_pattern"));
}

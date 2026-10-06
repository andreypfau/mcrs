// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

pub mod banner_pattern;
pub mod banner_pattern_tags;
pub mod block_transformer;
pub mod damage_type;
pub mod damage_type_tags;
pub mod decorated_pot_pattern;
pub mod dialog;
pub mod dialog_tags;
pub mod enchantment;
pub mod enchantment_tags;
pub mod instrument;
pub mod instrument_tags;
pub mod jukebox_song;
pub mod painting_variant;
pub mod painting_variant_tags;
pub mod sound_event;
pub mod trim_material;
pub mod trim_pattern;

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

pub const DAMAGE_TYPE: RegistryKey<crate::damage_type::DamageType> = RegistryKey::new(rl!("minecraft:damage_type"));
impl Registered for crate::damage_type::DamageType {
    const REGISTRY: RegistryKey<Self> = DAMAGE_TYPE;
}

pub const DECORATED_POT_PATTERN: RegistryKey<crate::decorated_pot_pattern::DecoratedPotPattern> = RegistryKey::new(rl!("minecraft:decorated_pot_pattern"));
impl Registered for crate::decorated_pot_pattern::DecoratedPotPattern {
    const REGISTRY: RegistryKey<Self> = DECORATED_POT_PATTERN;
}

pub const DIALOG: RegistryKey<crate::dialog::Dialog> = RegistryKey::new(rl!("minecraft:dialog"));
impl Registered for crate::dialog::Dialog {
    const REGISTRY: RegistryKey<Self> = DIALOG;
}

pub const ENCHANTMENT: RegistryKey<crate::enchantment::EnchantmentData> = RegistryKey::new(rl!("minecraft:enchantment"));
impl Registered for crate::enchantment::EnchantmentData {
    const REGISTRY: RegistryKey<Self> = ENCHANTMENT;
}

pub const INSTRUMENT: RegistryKey<crate::InstrumentValue> = RegistryKey::new(rl!("minecraft:instrument"));
impl Registered for crate::InstrumentValue {
    const REGISTRY: RegistryKey<Self> = INSTRUMENT;
}

pub const JUKEBOX_SONG: RegistryKey<crate::JukeboxSong> = RegistryKey::new(rl!("minecraft:jukebox_song"));
impl Registered for crate::JukeboxSong {
    const REGISTRY: RegistryKey<Self> = JUKEBOX_SONG;
}

pub const PAINTING_VARIANT: RegistryKey<crate::PaintingVariantValue> = RegistryKey::new(rl!("minecraft:painting_variant"));
impl Registered for crate::PaintingVariantValue {
    const REGISTRY: RegistryKey<Self> = PAINTING_VARIANT;
}

pub const SOUND_EVENT: RegistryKey<crate::SoundEvent> = RegistryKey::new(rl!("minecraft:sound_event"));
impl Registered for crate::SoundEvent {
    const REGISTRY: RegistryKey<Self> = SOUND_EVENT;
}

pub const TRIM_MATERIAL: RegistryKey<crate::TrimMaterial> = RegistryKey::new(rl!("minecraft:trim_material"));
impl Registered for crate::TrimMaterial {
    const REGISTRY: RegistryKey<Self> = TRIM_MATERIAL;
}

pub const TRIM_PATTERN: RegistryKey<crate::TrimPattern> = RegistryKey::new(rl!("minecraft:trim_pattern"));
impl Registered for crate::TrimPattern {
    const REGISTRY: RegistryKey<Self> = TRIM_PATTERN;
}

pub fn bindings() -> [TypeBinding; 12] {
    [
        BANNER_PATTERN.binding(),
        BLOCK_TRANSFORMER.binding(),
        DAMAGE_TYPE.binding(),
        DECORATED_POT_PATTERN.binding(),
        DIALOG.binding(),
        ENCHANTMENT.binding(),
        INSTRUMENT.binding(),
        JUKEBOX_SONG.binding(),
        PAINTING_VARIANT.binding(),
        SOUND_EVENT.binding(),
        TRIM_MATERIAL.binding(),
        TRIM_PATTERN.binding(),
    ]
}

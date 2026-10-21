pub mod attribute;
#[rustfmt::skip]
pub mod keys;
pub mod spawn;
pub mod variant;
pub mod villager;

pub use attribute::AttributeDefinition;

mcrs_minecraft_registries::declare_registries!(
    crate::variant::WolfSoundVariant => crate::keys::WOLF_SOUND_VARIANT, World, non_empty;
    crate::variant::PigSoundVariant => crate::keys::PIG_SOUND_VARIANT, World, non_empty;
    crate::variant::CatSoundVariant => crate::keys::CAT_SOUND_VARIANT, World, non_empty;
    crate::variant::CowSoundVariant => crate::keys::COW_SOUND_VARIANT, World, non_empty;
    crate::variant::ChickenSoundVariant => crate::keys::CHICKEN_SOUND_VARIANT, World, non_empty;
);

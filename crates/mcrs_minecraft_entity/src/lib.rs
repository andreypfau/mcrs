pub mod attribute;
#[rustfmt::skip]
pub mod keys;
pub mod spawn;
pub mod variant;
pub mod villager;

pub use attribute::AttributeDefinition;

mcrs_minecraft_registries::read_with_serde!(
    crate::variant::WolfSoundVariant,
    crate::variant::PigSoundVariant,
    crate::variant::CatSoundVariant,
    crate::variant::CowSoundVariant,
    crate::variant::ChickenSoundVariant
);

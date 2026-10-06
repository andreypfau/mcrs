// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

pub mod attribute;
pub mod cat_sound_variant;
pub mod cat_variant;
pub mod chicken_sound_variant;
pub mod chicken_variant;
pub mod cow_sound_variant;
pub mod cow_variant;
pub mod entity_type;
pub mod entity_type_tags;
pub mod frog_variant;
pub mod pig_sound_variant;
pub mod pig_variant;
pub mod villager_profession;
pub mod villager_type;
pub mod wolf_sound_variant;
pub mod wolf_variant;
pub mod zombie_nautilus_variant;

pub use attribute::Attribute;
pub use entity_type::EntityType;
pub use villager_profession::VillagerProfession;
pub use villager_type::VillagerType;

use mcrs_minecraft_core::{RegistryKey, TypeBinding, rl};
use mcrs_minecraft_registry::Registered;

pub const ATTRIBUTE: RegistryKey<crate::keys::Attribute> = RegistryKey::new(rl!("minecraft:attribute"));
impl Registered for crate::keys::Attribute {
    const REGISTRY: RegistryKey<Self> = ATTRIBUTE;
}

pub const CAT_SOUND_VARIANT: RegistryKey<crate::variant::CatSoundVariant> = RegistryKey::new(rl!("minecraft:cat_sound_variant"));
impl Registered for crate::variant::CatSoundVariant {
    const REGISTRY: RegistryKey<Self> = CAT_SOUND_VARIANT;
}

pub const CAT_VARIANT: RegistryKey<crate::variant::CatVariant> = RegistryKey::new(rl!("minecraft:cat_variant"));
impl Registered for crate::variant::CatVariant {
    const REGISTRY: RegistryKey<Self> = CAT_VARIANT;
}

pub const CHICKEN_SOUND_VARIANT: RegistryKey<crate::variant::ChickenSoundVariant> = RegistryKey::new(rl!("minecraft:chicken_sound_variant"));
impl Registered for crate::variant::ChickenSoundVariant {
    const REGISTRY: RegistryKey<Self> = CHICKEN_SOUND_VARIANT;
}

pub const CHICKEN_VARIANT: RegistryKey<crate::variant::ChickenVariant> = RegistryKey::new(rl!("minecraft:chicken_variant"));
impl Registered for crate::variant::ChickenVariant {
    const REGISTRY: RegistryKey<Self> = CHICKEN_VARIANT;
}

pub const COW_SOUND_VARIANT: RegistryKey<crate::variant::CowSoundVariant> = RegistryKey::new(rl!("minecraft:cow_sound_variant"));
impl Registered for crate::variant::CowSoundVariant {
    const REGISTRY: RegistryKey<Self> = COW_SOUND_VARIANT;
}

pub const COW_VARIANT: RegistryKey<crate::variant::CowVariant> = RegistryKey::new(rl!("minecraft:cow_variant"));
impl Registered for crate::variant::CowVariant {
    const REGISTRY: RegistryKey<Self> = COW_VARIANT;
}

pub const ENTITY_TYPE: RegistryKey<crate::keys::EntityType> = RegistryKey::new(rl!("minecraft:entity_type"));
impl Registered for crate::keys::EntityType {
    const REGISTRY: RegistryKey<Self> = ENTITY_TYPE;
}

pub const FROG_VARIANT: RegistryKey<crate::variant::FrogVariant> = RegistryKey::new(rl!("minecraft:frog_variant"));
impl Registered for crate::variant::FrogVariant {
    const REGISTRY: RegistryKey<Self> = FROG_VARIANT;
}

pub const PIG_SOUND_VARIANT: RegistryKey<crate::variant::PigSoundVariant> = RegistryKey::new(rl!("minecraft:pig_sound_variant"));
impl Registered for crate::variant::PigSoundVariant {
    const REGISTRY: RegistryKey<Self> = PIG_SOUND_VARIANT;
}

pub const PIG_VARIANT: RegistryKey<crate::variant::PigVariant> = RegistryKey::new(rl!("minecraft:pig_variant"));
impl Registered for crate::variant::PigVariant {
    const REGISTRY: RegistryKey<Self> = PIG_VARIANT;
}

pub const VILLAGER_PROFESSION: RegistryKey<crate::keys::VillagerProfession> = RegistryKey::new(rl!("minecraft:villager_profession"));
impl Registered for crate::keys::VillagerProfession {
    const REGISTRY: RegistryKey<Self> = VILLAGER_PROFESSION;
}

pub const VILLAGER_TYPE: RegistryKey<crate::keys::VillagerType> = RegistryKey::new(rl!("minecraft:villager_type"));
impl Registered for crate::keys::VillagerType {
    const REGISTRY: RegistryKey<Self> = VILLAGER_TYPE;
}

pub const WOLF_SOUND_VARIANT: RegistryKey<crate::variant::WolfSoundVariant> = RegistryKey::new(rl!("minecraft:wolf_sound_variant"));
impl Registered for crate::variant::WolfSoundVariant {
    const REGISTRY: RegistryKey<Self> = WOLF_SOUND_VARIANT;
}

pub const WOLF_VARIANT: RegistryKey<crate::variant::WolfVariant> = RegistryKey::new(rl!("minecraft:wolf_variant"));
impl Registered for crate::variant::WolfVariant {
    const REGISTRY: RegistryKey<Self> = WOLF_VARIANT;
}

pub const ZOMBIE_NAUTILUS_VARIANT: RegistryKey<crate::variant::ZombieNautilusVariant> = RegistryKey::new(rl!("minecraft:zombie_nautilus_variant"));
impl Registered for crate::variant::ZombieNautilusVariant {
    const REGISTRY: RegistryKey<Self> = ZOMBIE_NAUTILUS_VARIANT;
}

pub fn bindings() -> [TypeBinding; 16] {
    [
        ATTRIBUTE.binding(),
        CAT_SOUND_VARIANT.binding(),
        CAT_VARIANT.binding(),
        CHICKEN_SOUND_VARIANT.binding(),
        CHICKEN_VARIANT.binding(),
        COW_SOUND_VARIANT.binding(),
        COW_VARIANT.binding(),
        ENTITY_TYPE.binding(),
        FROG_VARIANT.binding(),
        PIG_SOUND_VARIANT.binding(),
        PIG_VARIANT.binding(),
        VILLAGER_PROFESSION.binding(),
        VILLAGER_TYPE.binding(),
        WOLF_SOUND_VARIANT.binding(),
        WOLF_VARIANT.binding(),
        ZOMBIE_NAUTILUS_VARIANT.binding(),
    ]
}

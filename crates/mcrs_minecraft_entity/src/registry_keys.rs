// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{Registered, RegistryKey, rl};

pub const ATTRIBUTE: RegistryKey<crate::attribute_keys::Attribute> =
    RegistryKey::new(rl!("minecraft:attribute"));
impl Registered for crate::attribute_keys::Attribute {
    const REGISTRY: RegistryKey<Self> = ATTRIBUTE;
}

pub const CAT_SOUND_VARIANT: RegistryKey<crate::variant::CatSoundVariant> =
    RegistryKey::new(rl!("minecraft:cat_sound_variant"));
impl Registered for crate::variant::CatSoundVariant {
    const REGISTRY: RegistryKey<Self> = CAT_SOUND_VARIANT;
}

pub const CAT_VARIANT: RegistryKey<crate::variant::CatVariant> =
    RegistryKey::new(rl!("minecraft:cat_variant"));
impl Registered for crate::variant::CatVariant {
    const REGISTRY: RegistryKey<Self> = CAT_VARIANT;
}

pub const CHICKEN_SOUND_VARIANT: RegistryKey<crate::variant::ChickenSoundVariant> =
    RegistryKey::new(rl!("minecraft:chicken_sound_variant"));
impl Registered for crate::variant::ChickenSoundVariant {
    const REGISTRY: RegistryKey<Self> = CHICKEN_SOUND_VARIANT;
}

pub const CHICKEN_VARIANT: RegistryKey<crate::variant::ChickenVariant> =
    RegistryKey::new(rl!("minecraft:chicken_variant"));
impl Registered for crate::variant::ChickenVariant {
    const REGISTRY: RegistryKey<Self> = CHICKEN_VARIANT;
}

pub const COW_SOUND_VARIANT: RegistryKey<crate::variant::CowSoundVariant> =
    RegistryKey::new(rl!("minecraft:cow_sound_variant"));
impl Registered for crate::variant::CowSoundVariant {
    const REGISTRY: RegistryKey<Self> = COW_SOUND_VARIANT;
}

pub const COW_VARIANT: RegistryKey<crate::variant::CowVariant> =
    RegistryKey::new(rl!("minecraft:cow_variant"));
impl Registered for crate::variant::CowVariant {
    const REGISTRY: RegistryKey<Self> = COW_VARIANT;
}

pub const ENTITY_TYPE: RegistryKey<crate::entity_type_keys::EntityType> =
    RegistryKey::new(rl!("minecraft:entity_type"));
impl Registered for crate::entity_type_keys::EntityType {
    const REGISTRY: RegistryKey<Self> = ENTITY_TYPE;
}

pub const FROG_VARIANT: RegistryKey<crate::variant::FrogVariant> =
    RegistryKey::new(rl!("minecraft:frog_variant"));
impl Registered for crate::variant::FrogVariant {
    const REGISTRY: RegistryKey<Self> = FROG_VARIANT;
}

pub const PIG_SOUND_VARIANT: RegistryKey<crate::variant::PigSoundVariant> =
    RegistryKey::new(rl!("minecraft:pig_sound_variant"));
impl Registered for crate::variant::PigSoundVariant {
    const REGISTRY: RegistryKey<Self> = PIG_SOUND_VARIANT;
}

pub const PIG_VARIANT: RegistryKey<crate::variant::PigVariant> =
    RegistryKey::new(rl!("minecraft:pig_variant"));
impl Registered for crate::variant::PigVariant {
    const REGISTRY: RegistryKey<Self> = PIG_VARIANT;
}

pub const SPAWN_CONDITION_TYPE: RegistryKey<crate::spawn_condition_type_keys::SpawnConditionType> =
    RegistryKey::new(rl!("minecraft:spawn_condition_type"));
impl Registered for crate::spawn_condition_type_keys::SpawnConditionType {
    const REGISTRY: RegistryKey<Self> = SPAWN_CONDITION_TYPE;
}

pub const VILLAGER_PROFESSION: RegistryKey<crate::villager_profession_keys::VillagerProfession> =
    RegistryKey::new(rl!("minecraft:villager_profession"));
impl Registered for crate::villager_profession_keys::VillagerProfession {
    const REGISTRY: RegistryKey<Self> = VILLAGER_PROFESSION;
}

pub const VILLAGER_TYPE: RegistryKey<crate::villager_type_keys::VillagerType> =
    RegistryKey::new(rl!("minecraft:villager_type"));
impl Registered for crate::villager_type_keys::VillagerType {
    const REGISTRY: RegistryKey<Self> = VILLAGER_TYPE;
}

pub const WOLF_SOUND_VARIANT: RegistryKey<crate::variant::WolfSoundVariant> =
    RegistryKey::new(rl!("minecraft:wolf_sound_variant"));
impl Registered for crate::variant::WolfSoundVariant {
    const REGISTRY: RegistryKey<Self> = WOLF_SOUND_VARIANT;
}

pub const WOLF_VARIANT: RegistryKey<crate::variant::WolfVariant> =
    RegistryKey::new(rl!("minecraft:wolf_variant"));
impl Registered for crate::variant::WolfVariant {
    const REGISTRY: RegistryKey<Self> = WOLF_VARIANT;
}

pub const STRUCTURE: RegistryKey<crate::spawn_condition::Structure> =
    RegistryKey::new(rl!("minecraft:worldgen/structure"));
impl Registered for crate::spawn_condition::Structure {
    const REGISTRY: RegistryKey<Self> = STRUCTURE;
}

pub const ZOMBIE_NAUTILUS_VARIANT: RegistryKey<crate::variant::ZombieNautilusVariant> =
    RegistryKey::new(rl!("minecraft:zombie_nautilus_variant"));
impl Registered for crate::variant::ZombieNautilusVariant {
    const REGISTRY: RegistryKey<Self> = ZOMBIE_NAUTILUS_VARIANT;
}

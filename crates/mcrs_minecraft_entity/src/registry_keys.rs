use mcrs_minecraft_resource::{Registered, RegistryKey, rl};

pub const CAT_SOUND_VARIANT: RegistryKey<crate::variant::CatSoundVariant> =
    <crate::variant::CatSoundVariant as Registered>::REGISTRY;
impl Registered for crate::variant::CatSoundVariant {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:cat_sound_variant"));
}

pub const CAT_VARIANT: RegistryKey<crate::variant::CatVariant> =
    <crate::variant::CatVariant as Registered>::REGISTRY;
impl Registered for crate::variant::CatVariant {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:cat_variant"));
}

pub const CHICKEN_SOUND_VARIANT: RegistryKey<crate::variant::ChickenSoundVariant> =
    <crate::variant::ChickenSoundVariant as Registered>::REGISTRY;
impl Registered for crate::variant::ChickenSoundVariant {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:chicken_sound_variant"));
}

pub const CHICKEN_VARIANT: RegistryKey<crate::variant::ChickenVariant> =
    <crate::variant::ChickenVariant as Registered>::REGISTRY;
impl Registered for crate::variant::ChickenVariant {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:chicken_variant"));
}

pub const COW_SOUND_VARIANT: RegistryKey<crate::variant::CowSoundVariant> =
    <crate::variant::CowSoundVariant as Registered>::REGISTRY;
impl Registered for crate::variant::CowSoundVariant {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:cow_sound_variant"));
}

pub const COW_VARIANT: RegistryKey<crate::variant::CowVariant> =
    <crate::variant::CowVariant as Registered>::REGISTRY;
impl Registered for crate::variant::CowVariant {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:cow_variant"));
}

pub const DAMAGE_TYPE: RegistryKey<crate::damage_type::DamageType> =
    <crate::damage_type::DamageType as Registered>::REGISTRY;
impl Registered for crate::damage_type::DamageType {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:damage_type"));
}

pub const ENTITY_TYPE: RegistryKey<crate::entity_type_keys::EntityType> =
    <crate::entity_type_keys::EntityType as Registered>::REGISTRY;
impl Registered for crate::entity_type_keys::EntityType {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:entity_type"));
}

pub const FROG_VARIANT: RegistryKey<crate::variant::FrogVariant> =
    <crate::variant::FrogVariant as Registered>::REGISTRY;
impl Registered for crate::variant::FrogVariant {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:frog_variant"));
}

pub const MOB_EFFECT: RegistryKey<crate::mob_effect_keys::MobEffect> =
    <crate::mob_effect_keys::MobEffect as Registered>::REGISTRY;
impl Registered for crate::mob_effect_keys::MobEffect {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:mob_effect"));
}

pub const PIG_SOUND_VARIANT: RegistryKey<crate::variant::PigSoundVariant> =
    <crate::variant::PigSoundVariant as Registered>::REGISTRY;
impl Registered for crate::variant::PigSoundVariant {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:pig_sound_variant"));
}

pub const PIG_VARIANT: RegistryKey<crate::variant::PigVariant> =
    <crate::variant::PigVariant as Registered>::REGISTRY;
impl Registered for crate::variant::PigVariant {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:pig_variant"));
}

pub const SPAWN_CONDITION_TYPE: RegistryKey<crate::spawn_condition_type_keys::SpawnConditionType> =
    <crate::spawn_condition_type_keys::SpawnConditionType as Registered>::REGISTRY;
impl Registered for crate::spawn_condition_type_keys::SpawnConditionType {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:spawn_condition_type"));
}

pub const VILLAGER_PROFESSION: RegistryKey<crate::villager_profession_keys::VillagerProfession> =
    <crate::villager_profession_keys::VillagerProfession as Registered>::REGISTRY;
impl Registered for crate::villager_profession_keys::VillagerProfession {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:villager_profession"));
}

pub const VILLAGER_TYPE: RegistryKey<crate::villager_type_keys::VillagerType> =
    <crate::villager_type_keys::VillagerType as Registered>::REGISTRY;
impl Registered for crate::villager_type_keys::VillagerType {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:villager_type"));
}

pub const WOLF_SOUND_VARIANT: RegistryKey<crate::variant::WolfSoundVariant> =
    <crate::variant::WolfSoundVariant as Registered>::REGISTRY;
impl Registered for crate::variant::WolfSoundVariant {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:wolf_sound_variant"));
}

pub const WOLF_VARIANT: RegistryKey<crate::variant::WolfVariant> =
    <crate::variant::WolfVariant as Registered>::REGISTRY;
impl Registered for crate::variant::WolfVariant {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:wolf_variant"));
}

pub const STRUCTURE: RegistryKey<crate::spawn_condition::Structure> =
    <crate::spawn_condition::Structure as Registered>::REGISTRY;
impl Registered for crate::spawn_condition::Structure {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:worldgen/structure"));
}

pub const ZOMBIE_NAUTILUS_VARIANT: RegistryKey<crate::variant::ZombieNautilusVariant> =
    <crate::variant::ZombieNautilusVariant as Registered>::REGISTRY;
impl Registered for crate::variant::ZombieNautilusVariant {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:zombie_nautilus_variant"));
}

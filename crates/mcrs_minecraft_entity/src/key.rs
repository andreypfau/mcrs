use mcrs_minecraft_core::registry_key::RegistryKey;
use mcrs_minecraft_core::{ResourceLocation, rl};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DamageType {}

impl RegistryKey for DamageType {
    const KEY: ResourceLocation<&'static str> = rl!("minecraft:damage_type");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WolfVariant {}

impl RegistryKey for WolfVariant {
    const KEY: ResourceLocation<&'static str> = rl!("minecraft:wolf_variant");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WolfSoundVariant {}

impl RegistryKey for WolfSoundVariant {
    const KEY: ResourceLocation<&'static str> = rl!("minecraft:wolf_sound_variant");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PigVariant {}

impl RegistryKey for PigVariant {
    const KEY: ResourceLocation<&'static str> = rl!("minecraft:pig_variant");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PigSoundVariant {}

impl RegistryKey for PigSoundVariant {
    const KEY: ResourceLocation<&'static str> = rl!("minecraft:pig_sound_variant");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CowVariant {}

impl RegistryKey for CowVariant {
    const KEY: ResourceLocation<&'static str> = rl!("minecraft:cow_variant");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CowSoundVariant {}

impl RegistryKey for CowSoundVariant {
    const KEY: ResourceLocation<&'static str> = rl!("minecraft:cow_sound_variant");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChickenVariant {}

impl RegistryKey for ChickenVariant {
    const KEY: ResourceLocation<&'static str> = rl!("minecraft:chicken_variant");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChickenSoundVariant {}

impl RegistryKey for ChickenSoundVariant {
    const KEY: ResourceLocation<&'static str> = rl!("minecraft:chicken_sound_variant");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZombieNautilusVariant {}

impl RegistryKey for ZombieNautilusVariant {
    const KEY: ResourceLocation<&'static str> = rl!("minecraft:zombie_nautilus_variant");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FrogVariant {}

impl RegistryKey for FrogVariant {
    const KEY: ResourceLocation<&'static str> = rl!("minecraft:frog_variant");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CatVariant {}

impl RegistryKey for CatVariant {
    const KEY: ResourceLocation<&'static str> = rl!("minecraft:cat_variant");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CatSoundVariant {}

impl RegistryKey for CatSoundVariant {
    const KEY: ResourceLocation<&'static str> = rl!("minecraft:cat_sound_variant");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MobEffect {}

impl RegistryKey for MobEffect {
    const KEY: ResourceLocation<&'static str> = rl!("minecraft:mob_effect");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameEvent {}

impl RegistryKey for GameEvent {
    const KEY: ResourceLocation<&'static str> = rl!("minecraft:game_event");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PointOfInterestType {}

impl RegistryKey for PointOfInterestType {
    const KEY: ResourceLocation<&'static str> = rl!("minecraft:point_of_interest_type");
}

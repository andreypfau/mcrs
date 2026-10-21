use crate::component::registry_ref::registry_key_component;
use crate::decorated_pot_pattern::DecoratedPotPattern;
use mcrs_minecraft_entity::keys::VillagerType;
use mcrs_minecraft_registries::{Refs, RegistryOf, Resolved, registry_value};

registry_key_component! {
    VillagerVariant(VillagerType) ["plains", "desert"],
    WolfVariant(RegistryOf<mcrs_minecraft_entity::variant::WolfVariant>) ["pale", "ashen"],
    WolfSoundVariant(RegistryOf<mcrs_minecraft_entity::variant::WolfSoundVariant>) ["classic", "big"],
    PigVariant(RegistryOf<mcrs_minecraft_entity::variant::PigVariant>) ["temperate", "cold"],
    PigSoundVariant(RegistryOf<mcrs_minecraft_entity::variant::PigSoundVariant>) ["classic", "mini"],
    CowVariant(RegistryOf<mcrs_minecraft_entity::variant::CowVariant>) ["temperate", "warm"],
    CowSoundVariant(RegistryOf<mcrs_minecraft_entity::variant::CowSoundVariant>) ["classic", "moody"],
    ChickenVariant(RegistryOf<mcrs_minecraft_entity::variant::ChickenVariant>) ["temperate", "cold"],
    ChickenSoundVariant(RegistryOf<mcrs_minecraft_entity::variant::ChickenSoundVariant>) ["classic", "picky"],
    ZombieNautilusVariant(RegistryOf<mcrs_minecraft_entity::variant::ZombieNautilusVariant>) ["temperate", "warm"],
    FrogVariant(RegistryOf<mcrs_minecraft_entity::variant::FrogVariant>) ["temperate", "warm"],
    CatVariant(RegistryOf<mcrs_minecraft_entity::variant::CatVariant>) ["tabby", "jellie"],
    CatSoundVariant(RegistryOf<mcrs_minecraft_entity::variant::CatSoundVariant>) ["classic", "royal"],
    ProvidesPotteryPattern(RegistryOf<DecoratedPotPattern>) ["angler", "skull"],
}

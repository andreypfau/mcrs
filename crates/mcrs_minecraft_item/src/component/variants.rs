use crate::component::registry_ref::registry_key_component;
use mcrs_minecraft_entity::VillagerType;
use mcrs_minecraft_keys::DecoratedPotPattern;

registry_key_component! {
    VillagerVariant(VillagerType) ["plains", "desert"],
    WolfVariant(mcrs_minecraft_keys::WolfVariant) ["pale", "ashen"],
    WolfSoundVariant(mcrs_minecraft_keys::WolfSoundVariant) ["classic", "big"],
    PigVariant(mcrs_minecraft_keys::PigVariant) ["temperate", "cold"],
    PigSoundVariant(mcrs_minecraft_keys::PigSoundVariant) ["classic", "mini"],
    CowVariant(mcrs_minecraft_keys::CowVariant) ["temperate", "warm"],
    CowSoundVariant(mcrs_minecraft_keys::CowSoundVariant) ["classic", "moody"],
    ChickenVariant(mcrs_minecraft_keys::ChickenVariant) ["temperate", "cold"],
    ChickenSoundVariant(mcrs_minecraft_keys::ChickenSoundVariant) ["classic", "picky"],
    ZombieNautilusVariant(mcrs_minecraft_keys::ZombieNautilusVariant) ["temperate", "warm"],
    FrogVariant(mcrs_minecraft_keys::FrogVariant) ["temperate", "warm"],
    CatVariant(mcrs_minecraft_keys::CatVariant) ["tabby", "jellie"],
    CatSoundVariant(mcrs_minecraft_keys::CatSoundVariant) ["classic", "royal"],
    ProvidesPotteryPattern(DecoratedPotPattern) ["angler", "skull"],
}

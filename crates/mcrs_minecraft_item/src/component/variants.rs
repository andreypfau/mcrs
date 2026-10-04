use crate::component::common::DecoratedPotPatternReg;
use crate::component::registry_ref::registry_key_component;
use mcrs_minecraft_entity::VillagerType;

registry_key_component! {
    VillagerVariant(VillagerType) ["plains", "desert"],
    WolfVariant(mcrs_minecraft_entity::WolfVariant) ["pale", "ashen"],
    WolfSoundVariant(mcrs_minecraft_entity::WolfSoundVariant) ["classic", "big"],
    PigVariant(mcrs_minecraft_entity::PigVariant) ["temperate", "cold"],
    PigSoundVariant(mcrs_minecraft_entity::PigSoundVariant) ["classic", "mini"],
    CowVariant(mcrs_minecraft_entity::CowVariant) ["temperate", "warm"],
    CowSoundVariant(mcrs_minecraft_entity::CowSoundVariant) ["classic", "moody"],
    ChickenVariant(mcrs_minecraft_entity::ChickenVariant) ["temperate", "cold"],
    ChickenSoundVariant(mcrs_minecraft_entity::ChickenSoundVariant) ["classic", "picky"],
    ZombieNautilusVariant(mcrs_minecraft_entity::ZombieNautilusVariant) ["temperate", "warm"],
    FrogVariant(mcrs_minecraft_entity::FrogVariant) ["temperate", "warm"],
    CatVariant(mcrs_minecraft_entity::CatVariant) ["tabby", "jellie"],
    CatSoundVariant(mcrs_minecraft_entity::CatSoundVariant) ["classic", "royal"],
    ProvidesPotteryPattern(DecoratedPotPatternReg) ["angler", "skull"],
}

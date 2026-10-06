use crate::component::registry_ref::registry_key_component;
use crate::decorated_pot_pattern::DecoratedPotPattern;
use mcrs_minecraft_entity::keys::VillagerType;

registry_key_component! {
    VillagerVariant(VillagerType) ["plains", "desert"],
    WolfVariant(mcrs_minecraft_entity::variant::WolfVariant) ["pale", "ashen"],
    WolfSoundVariant(mcrs_minecraft_entity::variant::WolfSoundVariant) ["classic", "big"],
    PigVariant(mcrs_minecraft_entity::variant::PigVariant) ["temperate", "cold"],
    PigSoundVariant(mcrs_minecraft_entity::variant::PigSoundVariant) ["classic", "mini"],
    CowVariant(mcrs_minecraft_entity::variant::CowVariant) ["temperate", "warm"],
    CowSoundVariant(mcrs_minecraft_entity::variant::CowSoundVariant) ["classic", "moody"],
    ChickenVariant(mcrs_minecraft_entity::variant::ChickenVariant) ["temperate", "cold"],
    ChickenSoundVariant(mcrs_minecraft_entity::variant::ChickenSoundVariant) ["classic", "picky"],
    ZombieNautilusVariant(mcrs_minecraft_entity::variant::ZombieNautilusVariant) ["temperate", "warm"],
    FrogVariant(mcrs_minecraft_entity::variant::FrogVariant) ["temperate", "warm"],
    CatVariant(mcrs_minecraft_entity::variant::CatVariant) ["tabby", "jellie"],
    CatSoundVariant(mcrs_minecraft_entity::variant::CatSoundVariant) ["classic", "royal"],
    ProvidesPotteryPattern(DecoratedPotPattern) ["angler", "skull"],
}

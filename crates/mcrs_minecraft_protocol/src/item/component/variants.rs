use crate::item::component::common::{
    CatSoundVariantReg, CatVariantReg, ChickenSoundVariantReg, ChickenVariantReg,
    CowSoundVariantReg, CowVariantReg, DecoratedPotPatternReg, FrogVariantReg, PigSoundVariantReg,
    PigVariantReg, VillagerTypeReg, WolfSoundVariantReg, WolfVariantReg, ZombieNautilusVariantReg,
};
use crate::item::component::registry_ref::registry_key_component;

registry_key_component! {
    VillagerVariant(VillagerTypeReg) ["plains", "desert"],
    WolfVariant(WolfVariantReg) ["pale", "ashen"],
    WolfSoundVariant(WolfSoundVariantReg) ["classic", "big"],
    PigVariant(PigVariantReg) ["temperate", "cold"],
    PigSoundVariant(PigSoundVariantReg) ["classic", "mini"],
    CowVariant(CowVariantReg) ["temperate", "warm"],
    CowSoundVariant(CowSoundVariantReg) ["classic", "moody"],
    ChickenVariant(ChickenVariantReg) ["temperate", "cold"],
    ChickenSoundVariant(ChickenSoundVariantReg) ["classic", "picky"],
    ZombieNautilusVariant(ZombieNautilusVariantReg) ["temperate", "warm"],
    FrogVariant(FrogVariantReg) ["temperate", "warm"],
    CatVariant(CatVariantReg) ["tabby", "jellie"],
    CatSoundVariant(CatSoundVariantReg) ["classic", "royal"],
    ProvidesPotteryPattern(DecoratedPotPatternReg) ["angler", "skull"],
}

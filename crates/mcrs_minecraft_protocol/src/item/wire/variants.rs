use crate::item::component::variants::*;
use crate::item::wire::newtype_wire;
use mcrs_minecraft_registries::Wire;

newtype_wire!(
    VillagerVariant<Wire>,
    WolfVariant<Wire>,
    WolfSoundVariant<Wire>,
    PigVariant<Wire>,
    PigSoundVariant<Wire>,
    CowVariant<Wire>,
    CowSoundVariant<Wire>,
    ChickenVariant<Wire>,
    ChickenSoundVariant<Wire>,
    ZombieNautilusVariant<Wire>,
    FrogVariant<Wire>,
    CatVariant<Wire>,
    CatSoundVariant<Wire>,
    ProvidesPotteryPattern<Wire>
);

use mcrs_minecraft_entity::variant::{
    ChickenSoundVariant, ChickenVariant, CowSoundVariant, CowVariant, PigSoundVariant, PigVariant,
};
use mcrs_minecraft_resource::Id;

/// The entity data of a mob that its type does not fix. A field that holds the default of its
/// entry sends no entry.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MobLooks {
    pub no_ai: bool,
    pub left_handed: bool,
    pub variant: MobVariant,
}

/// The variant entries of a farm animal. `None` is the default entry of the registry.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum MobVariant {
    #[default]
    Plain,
    Pig {
        variant: Option<Id<PigVariant>>,
        sound: Option<Id<PigSoundVariant>>,
    },
    Cow {
        variant: Option<Id<CowVariant>>,
        sound: Option<Id<CowSoundVariant>>,
    },
    Chicken {
        variant: Option<Id<ChickenVariant>>,
        sound: Option<Id<ChickenSoundVariant>>,
    },
    /// The wool of a sheep: bits 0 to 3 the colour, bit 4 sheared.
    Sheep { wool: u8 },
}

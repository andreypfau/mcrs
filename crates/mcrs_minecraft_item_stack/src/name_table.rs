#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum NameTable {
    Item,
    Enchantment,
    Attribute,
    Block,
    BlockTag,
    ComponentType,
    MapDecorationType,
    SoundEvent,
    EntityType,
    EntityTypeTag,
    MobEffect,
    Potion,
    TrimMaterial,
    TrimPattern,
    Instrument,
    BannerPattern,
    PaintingVariant,
}

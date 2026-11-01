/// The enchantment registry as an item stack names its entries: a stack holds enchantment ids
/// and keys only, and the definitions with their effects are `mcrs_minecraft_enchantment`'s.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Enchantment;

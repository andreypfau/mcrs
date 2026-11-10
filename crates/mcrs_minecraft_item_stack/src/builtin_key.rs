/// The component type of a built-in component.
// The variants are in the order of the type ids, and the derived order is the order of the patch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum BuiltinKey {
    MaxStackSize,
    MaxDamage,
    Damage,
    Unbreakable,
    Rarity,
    RepairCost,
}

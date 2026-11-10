use crate::BuiltinKey;
use crate::Damage;
use crate::MaxDamage;
use crate::MaxStackSize;
use crate::Rarity;
use crate::RepairCost;
use crate::Unbreakable;

/// One component of a built-in component type.
#[derive(Clone, Debug, PartialEq, Hash)]
#[non_exhaustive]
pub enum Builtin {
    MaxStackSize(MaxStackSize),
    MaxDamage(MaxDamage),
    Damage(Damage),
    Unbreakable(Unbreakable),
    Rarity(Rarity),
    RepairCost(RepairCost),
}

impl Builtin {
    pub(crate) fn key(&self) -> BuiltinKey {
        match self {
            Self::MaxStackSize(_) => BuiltinKey::MaxStackSize,
            Self::MaxDamage(_) => BuiltinKey::MaxDamage,
            Self::Damage(_) => BuiltinKey::Damage,
            Self::Unbreakable(_) => BuiltinKey::Unbreakable,
            Self::Rarity(_) => BuiltinKey::Rarity,
            Self::RepairCost(_) => BuiltinKey::RepairCost,
        }
    }
}

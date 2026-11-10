use crate::AttributeModifiers;
use crate::BuiltinKey;
use crate::CustomName;
use crate::Damage;
use crate::Enchantments;
use crate::Food;
use crate::ItemModel;
use crate::ItemName;
use crate::Lore;
use crate::MaxDamage;
use crate::MaxStackSize;
use crate::Rarity;
use crate::RepairCost;
use crate::Tool;
use crate::TooltipDisplay;
use crate::Unbreakable;

/// One component of a built-in component type.
#[derive(Clone, Debug, PartialEq, Hash)]
#[non_exhaustive]
pub enum Builtin {
    MaxStackSize(MaxStackSize),
    MaxDamage(MaxDamage),
    Damage(Damage),
    Unbreakable(Unbreakable),
    CustomName(CustomName),
    ItemName(ItemName),
    ItemModel(ItemModel),
    Lore(Lore),
    Rarity(Rarity),
    Enchantments(Enchantments),
    AttributeModifiers(AttributeModifiers),
    TooltipDisplay(TooltipDisplay),
    RepairCost(RepairCost),
    Food(Food),
    Tool(Tool),
}

impl Builtin {
    pub(crate) fn key(&self) -> BuiltinKey {
        match self {
            Self::MaxStackSize(_) => BuiltinKey::MaxStackSize,
            Self::MaxDamage(_) => BuiltinKey::MaxDamage,
            Self::Damage(_) => BuiltinKey::Damage,
            Self::Unbreakable(_) => BuiltinKey::Unbreakable,
            Self::CustomName(_) => BuiltinKey::CustomName,
            Self::ItemName(_) => BuiltinKey::ItemName,
            Self::ItemModel(_) => BuiltinKey::ItemModel,
            Self::Lore(_) => BuiltinKey::Lore,
            Self::Rarity(_) => BuiltinKey::Rarity,
            Self::Enchantments(_) => BuiltinKey::Enchantments,
            Self::AttributeModifiers(_) => BuiltinKey::AttributeModifiers,
            Self::TooltipDisplay(_) => BuiltinKey::TooltipDisplay,
            Self::RepairCost(_) => BuiltinKey::RepairCost,
            Self::Food(_) => BuiltinKey::Food,
            Self::Tool(_) => BuiltinKey::Tool,
        }
    }
}

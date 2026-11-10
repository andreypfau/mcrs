use mcrs_minecraft_resource::ResourceLocation;

/// The component type of a built-in component.
// The variants are in the order of the type ids, and the derived order is the order of the patch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum BuiltinKey {
    MaxStackSize,
    MaxDamage,
    Damage,
    Unbreakable,
    CustomName,
    ItemName,
    ItemModel,
    Lore,
    Rarity,
    Enchantments,
    AttributeModifiers,
    TooltipDisplay,
    RepairCost,
    Food,
    Tool,
}

const ALL: [BuiltinKey; 15] = [
    BuiltinKey::MaxStackSize,
    BuiltinKey::MaxDamage,
    BuiltinKey::Damage,
    BuiltinKey::Unbreakable,
    BuiltinKey::CustomName,
    BuiltinKey::ItemName,
    BuiltinKey::ItemModel,
    BuiltinKey::Lore,
    BuiltinKey::Rarity,
    BuiltinKey::Enchantments,
    BuiltinKey::AttributeModifiers,
    BuiltinKey::TooltipDisplay,
    BuiltinKey::RepairCost,
    BuiltinKey::Food,
    BuiltinKey::Tool,
];

pub(crate) const NAMESPACE: &str = "minecraft";

impl BuiltinKey {
    /// The path of the identifier of the component type. Its namespace is [`NAMESPACE`].
    pub(crate) const fn path(self) -> &'static str {
        match self {
            Self::MaxStackSize => "max_stack_size",
            Self::MaxDamage => "max_damage",
            Self::Damage => "damage",
            Self::Unbreakable => "unbreakable",
            Self::CustomName => "custom_name",
            Self::ItemName => "item_name",
            Self::ItemModel => "item_model",
            Self::Lore => "lore",
            Self::Rarity => "rarity",
            Self::Enchantments => "enchantments",
            Self::AttributeModifiers => "attribute_modifiers",
            Self::TooltipDisplay => "tooltip_display",
            Self::RepairCost => "repair_cost",
            Self::Food => "food",
            Self::Tool => "tool",
        }
    }

    pub(crate) fn from_location(location: ResourceLocation<'_>) -> Option<Self> {
        if location.namespace() != NAMESPACE {
            return None;
        }
        ALL.into_iter().find(|key| key.path() == location.path())
    }
}

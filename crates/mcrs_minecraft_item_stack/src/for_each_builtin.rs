#[doc(hidden)]
#[macro_export]
macro_rules! for_each_builtin {
    ($callback:ident) => {
        $callback! {
            MaxStackSize => "max_stack_size",
            MaxDamage => "max_damage",
            Damage => "damage",
            Unbreakable => "unbreakable",
            CustomName => "custom_name",
            ItemName => "item_name",
            ItemModel => "item_model",
            Lore => "lore",
            Rarity => "rarity",
            Enchantments => "enchantments",
            AttributeModifiers => "attribute_modifiers",
            TooltipDisplay => "tooltip_display",
            RepairCost => "repair_cost",
            Food => "food",
            Tool => "tool",
            MapId => "map_id",
            MapDecorations => "map_decorations",
            Profile => "profile",
            Equippable => "equippable",
        }
    };
}

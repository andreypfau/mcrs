/// Calls the macro `$callback` once with each built-in component type as `Type => "path"`.
///
/// The types are in the order of the type ids: this order is the index of each [`BuiltinKey`],
/// the order of the patch and saved data, so a new type goes at the end. The path is the path of
/// the identifier of the type in the `minecraft` namespace.
///
/// [`BuiltinKey`]: crate::BuiltinKey
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
        }
    };
}

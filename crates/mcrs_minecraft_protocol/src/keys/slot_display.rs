// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

mcrs_minecraft_registry::static_registry! {
    pub enum SlotDisplayType;
    Empty = "minecraft:empty",
    AnyFuel = "minecraft:any_fuel",
    WithAnyPotion = "minecraft:with_any_potion",
    OnlyWithComponent = "minecraft:only_with_component",
    Item = "minecraft:item",
    ItemStack = "minecraft:item_stack",
    Tag = "minecraft:tag",
    Dyed = "minecraft:dyed",
    SmithingTrim = "minecraft:smithing_trim",
    WithRemainder = "minecraft:with_remainder",
    Composite = "minecraft:composite",
}

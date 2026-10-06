// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

mcrs_minecraft_registry::static_registry! {
    pub enum LootPoolEntryType;
    Empty = "minecraft:empty",
    Item = "minecraft:item",
    LootTable = "minecraft:loot_table",
    Dynamic = "minecraft:dynamic",
    Tag = "minecraft:tag",
    Slots = "minecraft:slots",
    Alternatives = "minecraft:alternatives",
    Sequence = "minecraft:sequence",
    Group = "minecraft:group",
}

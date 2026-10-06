// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

mcrs_minecraft_registry::static_registry! {
    pub enum SlotSourceType;
    Group = "minecraft:group",
    Filtered = "minecraft:filtered",
    LimitSlots = "minecraft:limit_slots",
    SlotRange = "minecraft:slot_range",
    Contents = "minecraft:contents",
    Empty = "minecraft:empty",
}

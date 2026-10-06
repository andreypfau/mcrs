// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

mcrs_minecraft_registry::static_registry! {
    pub enum StatType;
    Mined = "minecraft:mined",
    Crafted = "minecraft:crafted",
    Used = "minecraft:used",
    Broken = "minecraft:broken",
    PickedUp = "minecraft:picked_up",
    Dropped = "minecraft:dropped",
    Killed = "minecraft:killed",
    KilledBy = "minecraft:killed_by",
    Custom = "minecraft:custom",
}

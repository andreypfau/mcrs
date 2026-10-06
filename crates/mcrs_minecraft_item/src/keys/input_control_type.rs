// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

mcrs_minecraft_registry::static_registry! {
    pub enum InputControlType;
    Boolean = "minecraft:boolean",
    NumberRange = "minecraft:number_range",
    SingleOption = "minecraft:single_option",
    Text = "minecraft:text",
}

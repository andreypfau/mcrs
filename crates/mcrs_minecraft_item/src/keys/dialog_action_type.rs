// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

mcrs_minecraft_registry::static_registry! {
    pub enum DialogActionType;
    OpenUrl = "minecraft:open_url",
    RunCommand = "minecraft:run_command",
    SuggestCommand = "minecraft:suggest_command",
    ShowDialog = "minecraft:show_dialog",
    ChangePage = "minecraft:change_page",
    CopyToClipboard = "minecraft:copy_to_clipboard",
    Custom = "minecraft:custom",
    DynamicRunCommand = "minecraft:dynamic/run_command",
    DynamicCustom = "minecraft:dynamic/custom",
}

// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_core::{StaticResourceLocation, rl};

pub const OPEN_URL: StaticResourceLocation = rl!("minecraft:open_url");
pub const RUN_COMMAND: StaticResourceLocation = rl!("minecraft:run_command");
pub const SUGGEST_COMMAND: StaticResourceLocation = rl!("minecraft:suggest_command");
pub const SHOW_DIALOG: StaticResourceLocation = rl!("minecraft:show_dialog");
pub const CHANGE_PAGE: StaticResourceLocation = rl!("minecraft:change_page");
pub const COPY_TO_CLIPBOARD: StaticResourceLocation = rl!("minecraft:copy_to_clipboard");
pub const CUSTOM: StaticResourceLocation = rl!("minecraft:custom");
pub const DYNAMIC_RUN_COMMAND: StaticResourceLocation = rl!("minecraft:dynamic/run_command");
pub const DYNAMIC_CUSTOM: StaticResourceLocation = rl!("minecraft:dynamic/custom");

pub const ENTRIES: &[StaticResourceLocation] = &[
    OPEN_URL,
    RUN_COMMAND,
    SUGGEST_COMMAND,
    SHOW_DIALOG,
    CHANGE_PAGE,
    COPY_TO_CLIPBOARD,
    CUSTOM,
    DYNAMIC_RUN_COMMAND,
    DYNAMIC_CUSTOM,
];

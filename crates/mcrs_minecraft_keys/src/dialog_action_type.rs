// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const CHANGE_PAGE: Id<crate::DialogActionType> = Id::from_static(4);
pub const COPY_TO_CLIPBOARD: Id<crate::DialogActionType> = Id::from_static(5);
pub const CUSTOM: Id<crate::DialogActionType> = Id::from_static(6);
pub const DYNAMIC_CUSTOM: Id<crate::DialogActionType> = Id::from_static(8);
pub const DYNAMIC_RUN_COMMAND: Id<crate::DialogActionType> = Id::from_static(7);
pub const OPEN_URL: Id<crate::DialogActionType> = Id::from_static(0);
pub const RUN_COMMAND: Id<crate::DialogActionType> = Id::from_static(1);
pub const SHOW_DIALOG: Id<crate::DialogActionType> = Id::from_static(3);
pub const SUGGEST_COMMAND: Id<crate::DialogActionType> = Id::from_static(2);

pub const NAMES: &[&str] = &[
    "minecraft:open_url",
    "minecraft:run_command",
    "minecraft:suggest_command",
    "minecraft:show_dialog",
    "minecraft:change_page",
    "minecraft:copy_to_clipboard",
    "minecraft:custom",
    "minecraft:dynamic/run_command",
    "minecraft:dynamic/custom",
];

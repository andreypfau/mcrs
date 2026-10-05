// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const CONFIRMATION: Id<crate::DialogType> = Id::from_static(4);
pub const DIALOG_LIST: Id<crate::DialogType> = Id::from_static(2);
pub const MULTI_ACTION: Id<crate::DialogType> = Id::from_static(3);
pub const NOTICE: Id<crate::DialogType> = Id::from_static(0);
pub const SERVER_LINKS: Id<crate::DialogType> = Id::from_static(1);

pub const NAMES: &[&str] = &[
    "minecraft:notice",
    "minecraft:server_links",
    "minecraft:dialog_list",
    "minecraft:multi_action",
    "minecraft:confirmation",
];

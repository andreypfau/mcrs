use bevy::prelude::*;
use mcrs_minecraft_core::VERSION_NAME;

use super::DebugScreenDisplayer;

const BRAND: &str = "mcrs";

pub fn display(mut displayer: ResMut<DebugScreenDisplayer>) {
    displayer.add_priority_line(format!(
        "Minecraft {VERSION_NAME} ({}/{BRAND})",
        env!("CARGO_PKG_VERSION")
    ));
}

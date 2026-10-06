// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

pub mod timeline;
pub mod timeline_tags;
pub mod world_clock;

use mcrs_minecraft_core::{RegistryKey, TypeBinding, rl};
use mcrs_minecraft_registry::Registered;

pub const TIMELINE: RegistryKey<crate::timeline::Timeline> = RegistryKey::new(rl!("minecraft:timeline"));
impl Registered for crate::timeline::Timeline {
    const REGISTRY: RegistryKey<Self> = TIMELINE;
}

pub const WORLD_CLOCK: RegistryKey<crate::world_clock::WorldClock> = RegistryKey::new(rl!("minecraft:world_clock"));
impl Registered for crate::world_clock::WorldClock {
    const REGISTRY: RegistryKey<Self> = WORLD_CLOCK;
}

pub fn bindings() -> [TypeBinding; 2] {
    [
        TIMELINE.binding(),
        WORLD_CLOCK.binding(),
    ]
}

// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

pub mod activity;
pub mod environment_attribute;
pub mod timeline;
pub mod timeline_tags;
pub mod world_clock;

pub use activity::Activity;
pub use environment_attribute::EnvironmentAttribute;

use mcrs_minecraft_core::{RegistryKey, TypeBinding, rl};
use mcrs_minecraft_registry::Registered;

pub const ACTIVITY: RegistryKey<crate::keys::Activity> = RegistryKey::new(rl!("minecraft:activity"));
impl Registered for crate::keys::Activity {
    const REGISTRY: RegistryKey<Self> = ACTIVITY;
}

pub const ENVIRONMENT_ATTRIBUTE: RegistryKey<crate::keys::EnvironmentAttribute> = RegistryKey::new(rl!("minecraft:environment_attribute"));
impl Registered for crate::keys::EnvironmentAttribute {
    const REGISTRY: RegistryKey<Self> = ENVIRONMENT_ATTRIBUTE;
}

pub const TIMELINE: RegistryKey<crate::timeline::Timeline> = RegistryKey::new(rl!("minecraft:timeline"));
impl Registered for crate::timeline::Timeline {
    const REGISTRY: RegistryKey<Self> = TIMELINE;
}

pub const WORLD_CLOCK: RegistryKey<crate::world_clock::WorldClock> = RegistryKey::new(rl!("minecraft:world_clock"));
impl Registered for crate::world_clock::WorldClock {
    const REGISTRY: RegistryKey<Self> = WORLD_CLOCK;
}

pub fn bindings() -> [TypeBinding; 4] {
    [
        ACTIVITY.binding(),
        ENVIRONMENT_ATTRIBUTE.binding(),
        TIMELINE.binding(),
        WORLD_CLOCK.binding(),
    ]
}

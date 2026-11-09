// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{Registered, RegistryKey, rl};

pub const ACTIVITY: RegistryKey<crate::activity_keys::Activity> =
    RegistryKey::new(rl!("minecraft:activity"));
impl Registered for crate::activity_keys::Activity {
    const REGISTRY: RegistryKey<Self> = ACTIVITY;
}

pub const ENVIRONMENT_ATTRIBUTE: RegistryKey<
    crate::environment_attribute_keys::EnvironmentAttribute,
> = RegistryKey::new(rl!("minecraft:environment_attribute"));
impl Registered for crate::environment_attribute_keys::EnvironmentAttribute {
    const REGISTRY: RegistryKey<Self> = ENVIRONMENT_ATTRIBUTE;
}

pub const TIMELINE: RegistryKey<crate::Timeline> =
    RegistryKey::new(rl!("minecraft:timeline"));
impl Registered for crate::Timeline {
    const REGISTRY: RegistryKey<Self> = TIMELINE;
}

pub const WORLD_CLOCK: RegistryKey<crate::WorldClock> =
    RegistryKey::new(rl!("minecraft:world_clock"));
impl Registered for crate::WorldClock {
    const REGISTRY: RegistryKey<Self> = WORLD_CLOCK;
}

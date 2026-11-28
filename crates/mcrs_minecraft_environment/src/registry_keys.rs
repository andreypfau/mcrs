use mcrs_minecraft_resource::{Registered, RegistryKey, rl};

pub const ACTIVITY: RegistryKey<crate::activity_keys::Activity> =
    <crate::activity_keys::Activity as Registered>::REGISTRY;
impl Registered for crate::activity_keys::Activity {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:activity"));
}

pub const ENVIRONMENT_ATTRIBUTE: RegistryKey<
    crate::environment_attribute_keys::EnvironmentAttribute,
> = <crate::environment_attribute_keys::EnvironmentAttribute as Registered>::REGISTRY;
impl Registered for crate::environment_attribute_keys::EnvironmentAttribute {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:environment_attribute"));
}

pub const TIMELINE: RegistryKey<crate::Timeline> = <crate::Timeline as Registered>::REGISTRY;
impl Registered for crate::Timeline {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:timeline"));
}

pub const WORLD_CLOCK: RegistryKey<crate::WorldClock> = <crate::WorldClock as Registered>::REGISTRY;
impl Registered for crate::WorldClock {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:world_clock"));
}

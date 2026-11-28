use mcrs_minecraft_resource::{Registered, RegistryKey, rl};

pub const DECORATED_POT_PATTERN: RegistryKey<crate::DecoratedPotPattern> =
    <crate::DecoratedPotPattern as Registered>::REGISTRY;
impl Registered for crate::DecoratedPotPattern {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:decorated_pot_pattern"));
}

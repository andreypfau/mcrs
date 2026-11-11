// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{Registered, RegistryKey, rl};

pub const DECORATED_POT_PATTERN: RegistryKey<crate::DecoratedPotPattern> =
    RegistryKey::new(rl!("minecraft:decorated_pot_pattern"));
impl Registered for crate::DecoratedPotPattern {
    const REGISTRY: RegistryKey<Self> = DECORATED_POT_PATTERN;
}

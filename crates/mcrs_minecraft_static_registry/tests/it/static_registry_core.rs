use mcrs_minecraft_resource::{Registered, RegistryKey, rl};
use mcrs_minecraft_static_registry::static_registry_core;

static_registry_core! {
    pub enum Shape;
    Square = "custom:square",
}

impl Registered for Shape {
    const REGISTRY: RegistryKey<Self> = RegistryKey::new(rl!("minecraft:shape"));
}

#[test]
fn an_entry_outside_the_minecraft_namespace_keeps_its_namespace() {
    assert_eq!(Shape::Square.location(), rl!("custom:square"));
}

use bevy_ecs::world::World;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_core::Versioned;
use mcrs_minecraft_item::Items;
use mcrs_minecraft_registries::{CurrentRegistries, Registries};
use std::sync::Arc;

#[derive(Clone)]
pub struct RegistrySnapshot {
    registries: Registries,
    blocks: Blocks,
    items: Items,
}

impl RegistrySnapshot {
    fn insert_into(&self, world: &mut World) {
        world.insert_resource(CurrentRegistries::new(self.registries.clone()));
        world.insert_resource(self.blocks.clone());
        world.insert_resource(self.items.clone());
    }
}

pub fn install_registries(world: &mut World, registries: Registries, blocks: Blocks, items: Items) {
    let snapshot = RegistrySnapshot {
        registries,
        blocks,
        items,
    };
    snapshot.insert_into(world);
    world.insert_resource(Versioned::new(Arc::new(snapshot)));
}

pub fn extract_registries(main: &World, sub: &mut World) {
    if let Some(snapshot) = Versioned::<RegistrySnapshot>::extract(main, sub) {
        snapshot.insert_into(sub);
    }
}

/// Swaps only the `Registries`. The old `Blocks` and `Items` stay from the previous snapshot,
/// so a dimension would see the new `Registries` beside items resolved against the old ones:
/// tests only.
#[cfg(feature = "test-support")]
pub fn replace_registries(world: &mut World, registries: Registries) {
    let mut snapshot =
        RegistrySnapshot::clone(world.resource::<Versioned<RegistrySnapshot>>().value());
    snapshot.registries = registries.clone();
    world.insert_resource(CurrentRegistries::new(registries));
    world
        .resource_mut::<Versioned<RegistrySnapshot>>()
        .replace(Arc::new(snapshot));
}

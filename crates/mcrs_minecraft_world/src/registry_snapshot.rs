use bevy_ecs::resource::Resource;
use bevy_ecs::world::World;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_core::Versioned;
use mcrs_minecraft_item::Items;
use mcrs_minecraft_registries::{CurrentRegistries, Registries};
use std::any::type_name;
use std::sync::Arc;

#[derive(Clone)]
pub struct RegistrySnapshot {
    registries: Arc<Registries>,
    blocks: Blocks,
    items: Items,
}

impl RegistrySnapshot {
    fn insert_into(&self, world: &mut World) {
        world.insert_resource(CurrentRegistries::new(Arc::clone(&self.registries)));
        world.insert_resource(self.blocks.clone());
        world.insert_resource(self.items.clone());
    }
}

fn held<T: Resource + Clone>(world: &World) -> T {
    world
        .get_resource::<T>()
        .cloned()
        .unwrap_or_else(|| panic!("{} is missing from the main world", type_name::<T>()))
}

pub fn install_registries(world: &mut World, registries: Arc<Registries>) {
    let snapshot = RegistrySnapshot {
        blocks: held(world),
        items: held(world),
        registries,
    };
    world.insert_resource(CurrentRegistries::new(Arc::clone(&snapshot.registries)));
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
    let registries = Arc::new(registries);
    let mut snapshot =
        RegistrySnapshot::clone(world.resource::<Versioned<RegistrySnapshot>>().value());
    snapshot.registries = Arc::clone(&registries);
    world.insert_resource(CurrentRegistries::new(registries));
    world
        .resource_mut::<Versioned<RegistrySnapshot>>()
        .replace(Arc::new(snapshot));
}

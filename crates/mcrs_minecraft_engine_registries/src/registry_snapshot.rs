use bevy_ecs::world::World;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_core::Versioned;
use mcrs_minecraft_item::Items;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_registries::{CurrentRegistries, Registries};
use std::sync::Arc;

#[derive(Clone)]
pub struct RegistrySnapshot {
    registries: Registries,
    blocks: Blocks,
    items: Items,
    item_table: Option<ItemTable>,
}

impl RegistrySnapshot {
    fn insert_into(&self, world: &mut World) {
        world.insert_resource(CurrentRegistries::new(self.registries.clone()));
        world.insert_resource(self.blocks.clone());
        world.insert_resource(self.items.clone());
        if let Some(table) = self.item_table {
            world.insert_resource(table);
        }
    }
}

pub fn install_registries(world: &mut World, registries: Registries, blocks: Blocks, items: Items) {
    let snapshot = RegistrySnapshot {
        registries,
        blocks,
        items,
        item_table: None,
    };
    snapshot.insert_into(world);
    world.insert_resource(Versioned::new(Arc::new(snapshot)));
}

pub fn install_item_table(
    world: &mut World,
    table: ItemTable,
) -> Result<(), RegistriesNotInstalled> {
    world.insert_resource(table);
    let Some(mut versioned) = world.get_resource_mut::<Versioned<RegistrySnapshot>>() else {
        return Err(RegistriesNotInstalled);
    };
    let mut snapshot = RegistrySnapshot::clone(versioned.value());
    snapshot.item_table = Some(table);
    versioned.replace(Arc::new(snapshot));
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("the item table is installed before the registries")]
pub struct RegistriesNotInstalled;

pub fn extract_registries(main: &World, sub: &mut World) {
    if let Some(snapshot) = Versioned::<RegistrySnapshot>::extract(main, sub) {
        snapshot.insert_into(sub);
    }
}

pub fn apply_registries(world: &mut World, snapshot: Versioned<RegistrySnapshot>) {
    snapshot.value().insert_into(world);
    world.insert_resource(snapshot);
}

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

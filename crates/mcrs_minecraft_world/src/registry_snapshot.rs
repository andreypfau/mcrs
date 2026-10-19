use crate::resolvers::ResolvedTables;
use bevy_ecs::resource::Resource;
use bevy_ecs::world::World;
use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_core::Versioned;
use mcrs_minecraft_enchantment::effects::EnchantmentEffects;
use mcrs_minecraft_entity::keys::EntityType;
use mcrs_minecraft_environment::timeline::Timeline;
use mcrs_minecraft_item::Items;
use mcrs_minecraft_item::enchantment::EnchantmentData;
use mcrs_minecraft_registries::{CurrentRegistries, Registries};
use mcrs_minecraft_registry::{Entries, Registry, RegistrySet};
use mcrs_minecraft_worldgen_structure::Structure;
use std::any::type_name;
use std::sync::Arc;

#[derive(Clone)]
pub struct RegistrySnapshot {
    registries: Arc<Registries>,
    set: RegistrySet,
    blocks: Blocks,
    items: Items,
    enchantments: Registry<EnchantmentData>,
    enchantment_data: Entries<EnchantmentData, EnchantmentData>,
    enchantment_effects: Entries<EnchantmentData, Option<EnchantmentEffects>>,
    biomes: Registry<Biome>,
    structures: Registry<Structure>,
    timelines: Registry<Timeline>,
    entity_types: Registry<EntityType>,
    resolved: ResolvedTables,
}

impl RegistrySnapshot {
    fn insert_into(&self, world: &mut World) {
        world.insert_resource(CurrentRegistries::new(Arc::clone(&self.registries)));
        world.insert_resource(self.set.clone());
        world.insert_resource(self.blocks.clone());
        world.insert_resource(self.items.clone());
        world.insert_resource(self.enchantments.clone());
        world.insert_resource(self.enchantment_data.clone());
        world.insert_resource(self.enchantment_effects.clone());
        world.insert_resource(self.biomes.clone());
        world.insert_resource(self.structures.clone());
        world.insert_resource(self.timelines.clone());
        world.insert_resource(self.entity_types.clone());
        self.resolved.insert_into(world);
    }
}

fn held<T: Resource + Clone>(world: &World) -> T {
    world
        .get_resource::<T>()
        .cloned()
        .unwrap_or_else(|| panic!("{} is missing from the main world", type_name::<T>()))
}

pub fn install_registries(
    world: &mut World,
    registries: Arc<Registries>,
    resolved: ResolvedTables,
) {
    let snapshot = RegistrySnapshot {
        set: held(world),
        blocks: held(world),
        items: held(world),
        enchantments: held(world),
        enchantment_data: held(world),
        enchantment_effects: held(world),
        biomes: held(world),
        structures: held(world),
        timelines: held(world),
        entity_types: held(world),
        registries,
        resolved,
    };
    world.insert_resource(CurrentRegistries::new(Arc::clone(&snapshot.registries)));
    world.insert_resource(Versioned::new(Arc::new(snapshot)));
}

pub fn extract_registries(main: &World, sub: &mut World) {
    if let Some(snapshot) = Versioned::<RegistrySnapshot>::extract(main, sub) {
        snapshot.insert_into(sub);
    }
}

/// Swaps only the `Registries` and the resolver outputs. The old `RegistrySet`, `Blocks`,
/// `Items`, `Registry` and `Entries` resources stay from the previous snapshot, so a dimension
/// would see the new `Registries` beside the old `RegistrySet`: tests only.
#[cfg(feature = "test-support")]
pub fn replace_registries(
    world: &mut World,
    registries: Registries,
) -> Result<(), mcrs_minecraft_registries::Diagnostics> {
    let resolved = crate::resolvers::run_resolvers(world, &registries)?;
    let registries = Arc::new(registries);
    let mut snapshot =
        RegistrySnapshot::clone(world.resource::<Versioned<RegistrySnapshot>>().value());
    snapshot.registries = Arc::clone(&registries);
    snapshot.resolved = resolved;
    world.insert_resource(CurrentRegistries::new(registries));
    world
        .resource_mut::<Versioned<RegistrySnapshot>>()
        .replace(Arc::new(snapshot));
    world.write_message(mcrs_minecraft_registries::RegistriesReloaded);
    Ok(())
}

pub mod condition;
pub mod context;
pub mod entry;

use crate::loaded::Loaded;
use bevy_app::{App, Plugin, PostStartup};
use bevy_ecs::prelude::{Commands, Res};
use bevy_ecs::resource::Resource;
use mcrs_minecraft_block::definition::{BlockDefinitions, Blocks, LootId};
use mcrs_minecraft_item::enchantment::EnchantmentData;
use mcrs_minecraft_item::loot::LootTable;
use mcrs_minecraft_loot::{LootCondition, LootTableBody};
use mcrs_minecraft_registry::{Entries, Id, Registry, RegistrySet, Tags};
use tracing::warn;

/// The loaded registries a loot table is rolled against.
#[derive(Resource, Clone)]
pub struct LootRegistries {
    pub tables: Registry<LootTable>,
    pub bodies: Entries<LootTable, LootTableBody>,
    pub predicate_names: Registry<LootCondition>,
    pub predicates: Entries<LootCondition, LootCondition>,
    pub predicate_tags: Tags<LootCondition>,
    pub enchantments: Registry<EnchantmentData>,
    pub enchantment_tags: Tags<EnchantmentData>,
}

impl LootRegistries {
    pub fn from_set(registries: &RegistrySet) -> Self {
        let loot = LootRegistries {
            tables: registries.loaded_registry(),
            bodies: registries.loaded_entries(),
            predicate_names: registries.loaded_registry(),
            predicates: registries.loaded_entries(),
            predicate_tags: registries.loaded_tags(),
            enchantments: registries.loaded_registry(),
            enchantment_tags: registries.loaded_tags(),
        };
        if let Some(id) = loot.self_referring_predicate() {
            panic!(
                "predicate `{}` refers to itself",
                loot.predicate_names
                    .name(id)
                    .expect("a loaded predicate has a name")
            );
        }
        loot
    }
}

/// The loot table of each table the block corpus names, by the id it interned.
#[derive(Resource, Default)]
pub struct BlockLootTables {
    tables: Box<[Option<Id<LootTable>>]>,
}

impl BlockLootTables {
    pub fn new(blocks: &BlockDefinitions, tables: &Registry<LootTable>) -> Self {
        let tables = (0..blocks.loot_table_count())
            .map(|index| {
                let name = blocks.loot_table(LootId(index as u16));
                let id = tables.by_name(name.as_str());
                if id.is_none() {
                    warn!(table = %name, "a block names a loot table the data pack lacks");
                }
                id
            })
            .collect();
        BlockLootTables { tables }
    }

    pub fn get(&self, loot: LootId) -> Option<Id<LootTable>> {
        self.tables.get(usize::from(loot.0)).copied().flatten()
    }
}

fn index_block_loot_tables(mut commands: Commands, blocks: Res<Blocks>, loot: Res<LootRegistries>) {
    commands.insert_resource(BlockLootTables::new(&blocks, &loot.tables));
}

pub struct LootPlugin;

impl Plugin for LootPlugin {
    fn build(&self, app: &mut App) {
        let registries = app
            .world()
            .get_resource::<RegistrySet>()
            .expect("the loaded registries precede the loot plugin");
        let loot = LootRegistries::from_set(registries);
        app.insert_resource(loot);
        app.init_resource::<BlockLootTables>();
        app.add_systems(PostStartup, index_block_loot_tables);
    }
}

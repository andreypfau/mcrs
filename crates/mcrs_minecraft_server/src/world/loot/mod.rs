pub mod condition;
pub mod context;
pub mod entry;

use crate::world::loot::condition::LootCondition;
use crate::world::loot::context::{BlockBreakContext, LootDrop};
use crate::world::loot::entry::LootEntry;
use bevy_app::{App, Plugin, PostStartup, Update};
use bevy_asset::io::Reader;
use bevy_asset::{
    Asset, AssetApp, AssetEvent, AssetLoader, AssetServer, Assets, Handle, LoadContext,
    VisitAssetDependencies,
};
use bevy_ecs::message::MessageReader;
use bevy_ecs::prelude::ResMut;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::Res;
use bevy_reflect::TypePath;
use mcrs_minecraft_assets::asset::read_all;
use mcrs_minecraft_block::definition::{BlockDefinitions, Blocks, LootId};
use mcrs_minecraft_item::enchantment::EnchantmentData;
use mcrs_minecraft_registry::StaticRegistry;
use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{debug, info};

#[derive(Debug, Clone, Deserialize)]
pub struct LootTable {
    #[serde(rename = "type")]
    pub table_type: String,
    #[serde(default)]
    pub pools: Vec<LootPool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LootPool {
    pub rolls: u32,
    pub entries: Vec<LootEntry>,
    #[serde(default)]
    pub conditions: Vec<LootCondition>,
}

impl LootTable {
    pub fn evaluate(&self, ctx: &BlockBreakContext) -> Vec<LootDrop> {
        let mut drops = Vec::new();
        for pool in &self.pools {
            if !pool.conditions.iter().all(|c| c.check(ctx)) {
                continue;
            }
            for _ in 0..pool.rolls {
                drops.extend(pool.entries.iter().filter_map(|entry| entry.evaluate(ctx)));
            }
        }
        drops
    }

    fn drop_unknown_enchantments(&mut self, registry: &StaticRegistry<EnchantmentData>) {
        for pool in &mut self.pools {
            for entry in &mut pool.entries {
                entry.drop_unknown_enchantments(registry);
            }
            for condition in &mut pool.conditions {
                condition.drop_unknown_enchantments(registry);
            }
        }
    }
}

// ============================================================================
// Bevy Asset Types
// ============================================================================

#[derive(Debug, TypePath)]
pub struct LootTableAsset {
    pub loot: LootId,
    pub table_id: String,
    pub table: LootTable,
}

impl Asset for LootTableAsset {}

impl VisitAssetDependencies for LootTableAsset {
    fn visit_dependencies(&self, _visit: &mut impl FnMut(bevy_asset::UntypedAssetId)) {}
}

#[derive(Default, TypePath)]
pub struct LootTableLoader;

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct LootTableLoaderSettings {
    pub loot: Option<u16>,
    pub table_id: Option<String>,
}

#[derive(Debug, Error)]
pub enum LootTableLoaderError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON parse error: {0}")]
    Json(String),
    #[error("missing loot table identity in loader settings")]
    MissingTableId,
}

impl AssetLoader for LootTableLoader {
    type Asset = LootTableAsset;
    type Settings = LootTableLoaderSettings;
    type Error = LootTableLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let (Some(loot), Some(table_id)) = (settings.loot, settings.table_id.clone()) else {
            return Err(LootTableLoaderError::MissingTableId);
        };

        let bytes = read_all(reader).await?;

        let table: LootTable = serde_json::from_slice(&bytes)
            .map_err(|e| LootTableLoaderError::Json(e.to_string()))?;

        debug!(table = %table_id, pools = table.pools.len(), "loaded loot table");

        Ok(LootTableAsset {
            loot: LootId(loot),
            table_id,
            table,
        })
    }
}

// ============================================================================
// Resources & Plugin
// ============================================================================

/// Loot tables keyed by the table the corpus names, not by the block: a table
/// shared by several blocks is loaded once, and a block that names none has no
/// entry to miss.
#[derive(Resource, Default)]
pub struct BlockLootTables {
    pub tables: FxHashMap<LootId, LootTable>,
    pending: FxHashSet<LootId>,
    /// Keeps asset handles alive while loading.
    handles: Vec<Handle<LootTableAsset>>,
}

impl BlockLootTables {
    /// Request the table the corpus interned as `loot`. Returns true if it is
    /// already resolved.
    pub fn request(
        &mut self,
        loot: LootId,
        blocks: &BlockDefinitions,
        asset_server: &AssetServer,
    ) -> bool {
        if self.tables.contains_key(&loot) {
            return true;
        }
        if !self.pending.insert(loot) {
            return false;
        }
        let table_id = blocks.loot_table(loot);
        // `minecraft:blocks/stone` names `minecraft/loot_table/blocks/stone.json`.
        let path = format!(
            "{}/loot_table/{}.json",
            table_id.namespace(),
            table_id.path()
        );
        let settings = LootTableLoaderSettings {
            loot: Some(loot.0),
            table_id: Some(table_id.as_str().to_owned()),
        };
        debug!(table = %table_id, path = %path, "requesting loot table load");
        let handle: Handle<LootTableAsset> = asset_server
            .load_builder()
            .with_settings(move |s: &mut LootTableLoaderSettings| {
                *s = settings.clone();
            })
            .load(&path);
        self.handles.push(handle);
        false
    }
}

fn request_loot_tables_for_corpus(
    blocks: Res<Blocks>,
    asset_server: Res<AssetServer>,
    mut block_loot_tables: ResMut<BlockLootTables>,
) {
    for index in 0..blocks.loot_table_count() {
        block_loot_tables.request(LootId(index as u16), &blocks, &asset_server);
    }
    info!(
        requested = block_loot_tables.pending.len(),
        "requested block loot tables"
    );
}

fn process_loaded_loot_tables(
    mut events: MessageReader<AssetEvent<LootTableAsset>>,
    assets: Res<Assets<LootTableAsset>>,
    enchantment_registry: Res<StaticRegistry<EnchantmentData>>,
    mut block_loot_tables: ResMut<BlockLootTables>,
) {
    for event in events.read() {
        if let AssetEvent::LoadedWithDependencies { id } = event
            && let Some(asset) = assets.get(*id)
        {
            let mut resolved = asset.table.clone();
            resolved.drop_unknown_enchantments(&enchantment_registry);
            debug!(
                table = %asset.table_id,
                pools = resolved.pools.len(),
                "resolved loot table"
            );
            block_loot_tables.pending.remove(&asset.loot);
            block_loot_tables.tables.insert(asset.loot, resolved);
        }
    }
}

pub struct LootPlugin;

impl Plugin for LootPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<LootTableAsset>()
            .register_asset_loader(LootTableLoader);
        app.init_resource::<BlockLootTables>();
        app.add_systems(PostStartup, request_loot_tables_for_corpus);
        app.add_systems(Update, process_loaded_loot_tables);
    }
}

#[cfg(test)]
mod tests {
    use super::LootTable;

    #[test]
    fn every_shipped_block_loot_table_parses() {
        let dir = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/minecraft/loot_table/blocks"
        );
        let mut count = 0;
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let bytes = std::fs::read(&path).unwrap();
            serde_json::from_slice::<LootTable>(&bytes)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            count += 1;
        }
        assert!(count > 1000, "only {count} block loot tables");
    }
}

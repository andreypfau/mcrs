pub mod condition;
pub mod context;
pub mod entry;
pub mod function;

use crate::enchantment::EnchantmentData;
use crate::world::loot::condition::{LootCondition, LootConditionProto};
use crate::world::loot::context::{BlockBreakContext, LootDrop};
use crate::world::loot::entry::LootEntryProto;
use crate::world::loot::function::LootFunctionProto;
use crate::world::block::Block;
use bevy_app::{App, Plugin, PostStartup, Update};
use bevy_asset::io::Reader;
use bevy_asset::{Asset, AssetApp, AssetEvent, AssetLoader, AssetServer, Assets, Handle, LoadContext, VisitAssetDependencies};
use bevy_ecs::message::MessageReader;
use bevy_ecs::prelude::ResMut;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::Res;
use bevy_reflect::TypePath;
use mcrs_registry::Registry;
use rustc_hash::FxHashSet;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;
use thiserror::Error;
use tracing::{debug, info, warn};
use mcrs_protocol::Ident;

// ============================================================================
// Proto types (JSON deserialization)
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
pub struct LootTableProto {
    #[serde(rename = "type")]
    pub table_type: String,
    #[serde(default)]
    pub pools: Vec<LootPoolProto>,
    #[serde(default)]
    pub random_sequence: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LootPoolProto {
    pub rolls: serde_json::Value,
    #[serde(default)]
    pub bonus_rolls: f32,
    pub entries: Vec<LootEntryProto>,
    #[serde(default)]
    pub conditions: Vec<LootConditionProto>,
    #[serde(default)]
    pub functions: Vec<LootFunctionProto>,
}

// ============================================================================
// Resolved runtime types
// ============================================================================

#[derive(Debug, Clone)]
pub struct LootTable {
    pub pools: Vec<LootPool>,
}

#[derive(Debug, Clone)]
pub struct LootPool {
    pub rolls: u32,
    pub entries: Vec<LootEntry>,
    pub conditions: Vec<LootCondition>,
}

#[derive(Debug, Clone)]
pub enum LootEntry {
    Item {
        name: Ident<String>,
        conditions: Vec<LootCondition>,
    },
    Alternatives {
        children: Vec<LootEntry>,
        conditions: Vec<LootCondition>,
    },
    Empty {
        conditions: Vec<LootCondition>,
    },
}

// ============================================================================
// Resolution: Proto -> Resolved
// ============================================================================

impl LootTableProto {
    pub fn resolve(&self, enchantment_registry: &Registry<EnchantmentData>) -> LootTable {
        LootTable {
            pools: self
                .pools
                .iter()
                .map(|p| p.resolve(enchantment_registry))
                .collect(),
        }
    }
}

impl LootPoolProto {
    fn resolve(&self, enchantment_registry: &Registry<EnchantmentData>) -> LootPool {
        let rolls = match &self.rolls {
            serde_json::Value::Number(n) => n.as_u64().unwrap_or(1) as u32,
            _ => 1,
        };
        LootPool {
            rolls,
            entries: self
                .entries
                .iter()
                .map(|e| resolve_entry(e, enchantment_registry))
                .collect(),
            conditions: self
                .conditions
                .iter()
                .map(|c| resolve_condition(c, enchantment_registry))
                .collect(),
        }
    }
}

fn resolve_entry(entry: &LootEntryProto, enchantment_registry: &Registry<EnchantmentData>) -> LootEntry {
    match entry {
        LootEntryProto::Item {
            name, conditions, ..
        } => LootEntry::Item {
            name: name.clone(),
            conditions: conditions
                .iter()
                .map(|c| resolve_condition(c, enchantment_registry))
                .collect(),
        },
        LootEntryProto::Alternatives {
            children,
            conditions,
        } => LootEntry::Alternatives {
            children: children
                .iter()
                .map(|e| resolve_entry(e, enchantment_registry))
                .collect(),
            conditions: conditions
                .iter()
                .map(|c| resolve_condition(c, enchantment_registry))
                .collect(),
        },
        LootEntryProto::Empty { conditions } => LootEntry::Empty {
            conditions: conditions
                .iter()
                .map(|c| resolve_condition(c, enchantment_registry))
                .collect(),
        },
        LootEntryProto::Unknown => LootEntry::Empty {
            conditions: vec![],
        },
    }
}

fn resolve_condition(
    condition: &LootConditionProto,
    enchantment_registry: &Registry<EnchantmentData>,
) -> LootCondition {
    match condition {
        LootConditionProto::MatchTool { predicate } => {
            if let Some(predicates) = &predicate.predicates {
                if let Some(enchantments) = &predicates.enchantments {
                    if let Some(first) = enchantments.first() {
                        let enchantment_id = &first.enchantments;
                        if let Some((index, _)) = enchantment_registry.get_full(
                            enchantment_id.clone()
                        ) {
                            let min_level = first
                                .levels
                                .as_ref()
                                .and_then(|l| l.min)
                                .unwrap_or(1);
                            return LootCondition::MatchToolEnchantment {
                                enchantment_registry_index: index as u16,
                                min_level,
                            };
                        }
                        warn!(
                            enchantment = %enchantment_id,
                            "Enchantment not found in registry, condition will always be false"
                        );
                    }
                }
            }
            LootCondition::AlwaysTrue
        }
        LootConditionProto::SurvivesExplosion {} => LootCondition::SurvivesExplosion,
        LootConditionProto::Inverted { term } => {
            LootCondition::Inverted(Box::new(resolve_condition(term, enchantment_registry)))
        }
        LootConditionProto::AnyOf { terms } => LootCondition::AnyOf(
            terms
                .iter()
                .map(|t| resolve_condition(t, enchantment_registry))
                .collect(),
        ),
        LootConditionProto::AllOf { terms } => LootCondition::AllOf(
            terms
                .iter()
                .map(|t| resolve_condition(t, enchantment_registry))
                .collect(),
        ),
        LootConditionProto::Unknown => LootCondition::AlwaysTrue,
    }
}

// ============================================================================
// Evaluation
// ============================================================================

impl LootTable {
    pub fn evaluate(&self, ctx: &BlockBreakContext) -> Vec<LootDrop> {
        let mut drops = Vec::new();
        for pool in &self.pools {
            if !pool.conditions.iter().all(|c| c.check(ctx)) {
                continue;
            }
            for _ in 0..pool.rolls {
                for entry in &pool.entries {
                    if let Some(drop) = evaluate_entry(entry, ctx) {
                        drops.push(drop);
                    }
                }
            }
        }
        drops
    }
}

fn evaluate_entry(entry: &LootEntry, ctx: &BlockBreakContext) -> Option<LootDrop> {
    match entry {
        LootEntry::Item { name, conditions } => {
            if conditions.iter().all(|c| c.check(ctx)) {
                Some(LootDrop {
                    item_name: name.clone(),
                    count: 1,
                })
            } else {
                None
            }
        }
        LootEntry::Alternatives {
            children,
            conditions,
        } => {
            if !conditions.iter().all(|c| c.check(ctx)) {
                return None;
            }
            for child in children {
                if let Some(drop) = evaluate_entry(child, ctx) {
                    return Some(drop);
                }
            }
            None
        }
        LootEntry::Empty { conditions } => {
            if conditions.iter().all(|c| c.check(ctx)) {
                None // Empty entry drops nothing
            } else {
                None
            }
        }
    }
}

// ============================================================================
// Bevy Asset Types
// ============================================================================

#[derive(Debug, TypePath)]
pub struct LootTableAsset {
    pub block_id: Ident<String>,
    pub proto: LootTableProto,
}

impl Asset for LootTableAsset {}

impl VisitAssetDependencies for LootTableAsset {
    fn visit_dependencies(&self, _visit: &mut impl FnMut(bevy_asset::UntypedAssetId)) {}
}

#[derive(Default, TypePath)]
pub struct LootTableLoader;

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct LootTableLoaderSettings {
    pub block_id: Option<String>,
}

#[derive(Debug, Error)]
pub enum LootTableLoaderError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON parse error: {0}")]
    Json(String),
    #[error("Missing block_id in loader settings")]
    MissingBlockId,
    #[error("Invalid block identifier: {0}")]
    InvalidIdent(String),
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
        let block_id_str = settings
            .block_id
            .as_deref()
            .ok_or(LootTableLoaderError::MissingBlockId)?;
        let block_id = Ident::from_str(block_id_str)
            .map_err(|_| LootTableLoaderError::InvalidIdent(block_id_str.to_string()))?;

        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;

        let proto: LootTableProto = serde_json::from_slice(&bytes)
            .map_err(|e| LootTableLoaderError::Json(e.to_string()))?;

        debug!(
            block = %block_id,
            pools = proto.pools.len(),
            "Loaded loot table"
        );

        Ok(LootTableAsset { block_id, proto })
    }
}

// ============================================================================
// Resources & Plugin
// ============================================================================

#[derive(Resource, Default)]
pub struct BlockLootTables {
    pub tables: HashMap<Ident<String>, LootTable>,
    /// Tracks block names whose loot tables have been requested but not yet loaded.
    pending: FxHashSet<Ident<String>>,
    /// Keeps asset handles alive while loading.
    handles: Vec<Handle<LootTableAsset>>,
}

impl BlockLootTables {
    /// Request loading a loot table for the given block identifier (e.g. "minecraft:stone").
    /// Returns true if the table is already loaded, false if loading was triggered or is in progress.
    pub fn request(&mut self, block_id: &Ident<String>, asset_server: &AssetServer) -> bool {
        if self.tables.contains_key(block_id) {
            return true;
        }
        if self.pending.contains(block_id) {
            return false;
        }
        // "minecraft:stone" -> asset path "minecraft/loot_table/blocks/stone.json"
        let path = format!(
            "{}/loot_table/blocks/{}.json",
            block_id.namespace(),
            block_id.path()
        );
        let settings = LootTableLoaderSettings {
            block_id: Some(block_id.as_str().to_string()),
        };
        debug!(block = %block_id, path = %path, "Requesting loot table load");
        let handle: Handle<LootTableAsset> =
            asset_server.load_with_settings(&path, move |s: &mut LootTableLoaderSettings| {
                *s = settings.clone();
            });
        self.handles.push(handle);
        self.pending.insert(block_id.clone());
        false
    }
}

fn request_loot_tables_for_registered_blocks(
    block_registry: Res<Registry<&'static Block>>,
    asset_server: Res<AssetServer>,
    mut block_loot_tables: ResMut<BlockLootTables>,
) {
    for (id, _block) in block_registry.iter_entries() {
        block_loot_tables.request(id, &asset_server);
    }
    info!(
        requested = block_loot_tables.pending.len(),
        "Requested loot tables for registered blocks"
    );
}

fn process_loaded_loot_tables(
    mut events: MessageReader<AssetEvent<LootTableAsset>>,
    assets: Res<Assets<LootTableAsset>>,
    enchantment_registry: Res<Registry<EnchantmentData>>,
    mut block_loot_tables: ResMut<BlockLootTables>,
) {
    for event in events.read() {
        match event {
            AssetEvent::LoadedWithDependencies { id } => {
                if let Some(asset) = assets.get(*id) {
                    let resolved = asset.proto.resolve(&enchantment_registry);
                    info!(
                        block = %asset.block_id,
                        pools = resolved.pools.len(),
                        "Resolved loot table"
                    );
                    block_loot_tables.pending.remove(&asset.block_id);
                    block_loot_tables
                        .tables
                        .insert(asset.block_id.clone(), resolved);
                }
            }
            _ => {}
        }
    }
}

pub struct LootPlugin;

impl Plugin for LootPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<LootTableAsset>()
            .register_asset_loader(LootTableLoader);
        app.init_resource::<BlockLootTables>();
        app.add_systems(PostStartup, request_loot_tables_for_registered_blocks);
        app.add_systems(Update, process_loaded_loot_tables);
    }
}

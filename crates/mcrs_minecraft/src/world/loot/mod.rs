pub mod condition;
pub mod context;
pub mod entry;
pub mod function;

use crate::enchantment::EnchantmentData;
use crate::world::loot::condition::{LootCondition, LootConditionProto};
use crate::world::loot::context::{BlockBreakContext, LootDrop};
use crate::world::loot::entry::LootEntryProto;
use crate::world::loot::function::LootFunctionProto;
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
use mcrs_core::StaticRegistry;
use mcrs_protocol::Ident;
use mcrs_vanilla::block::definition::{BlockDefinitions, Blocks, LootId};
use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{debug, info, warn};

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
    pub fn resolve(&self, enchantment_registry: &StaticRegistry<EnchantmentData>) -> LootTable {
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
    fn resolve(&self, enchantment_registry: &StaticRegistry<EnchantmentData>) -> LootPool {
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

fn resolve_entry(
    entry: &LootEntryProto,
    enchantment_registry: &StaticRegistry<EnchantmentData>,
) -> LootEntry {
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
        LootEntryProto::Unknown => LootEntry::Empty { conditions: vec![] },
    }
}

fn resolve_condition(
    condition: &LootConditionProto,
    enchantment_registry: &StaticRegistry<EnchantmentData>,
) -> LootCondition {
    match condition {
        LootConditionProto::MatchTool { predicate } => {
            if let Some(predicates) = &predicate.predicates
                && let Some(enchantments) = &predicates.enchantments
                && let Some(first) = enchantments.first()
            {
                let enchantment_id = &first.enchantments;
                if let Some(static_id) = enchantment_registry.id_of(enchantment_id.as_str()) {
                    let min_level = first.levels.as_ref().and_then(|l| l.min).unwrap_or(1);
                    return LootCondition::MatchToolEnchantment {
                        enchantment_registry_index: static_id.raw() as u16,
                        min_level,
                    };
                }
                warn!(
                    enchantment = %enchantment_id,
                    "Enchantment not found in registry, condition will always be false"
                );
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
        LootEntry::Empty { conditions: _ } => {
            // Empty entry never produces a drop regardless of condition outcome.
            None
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

        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;

        let proto: LootTableProto = serde_json::from_slice(&bytes)
            .map_err(|e| LootTableLoaderError::Json(e.to_string()))?;

        debug!(table = %table_id, pools = proto.pools.len(), "loaded loot table");

        Ok(LootTableAsset {
            loot: LootId(loot),
            table_id,
            proto,
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
        let handle: Handle<LootTableAsset> =
            asset_server.load_with_settings(&path, move |s: &mut LootTableLoaderSettings| {
                *s = settings.clone();
            });
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
            let resolved = asset.proto.resolve(&enchantment_registry);
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

use crate::tag::block::TagRegistry;
use crate::tag::loader::{ResourcePackTags, TagFileLoaderSettings};
use bevy_app::{App, Plugin, PostStartup, Update};
use bevy_asset::{AssetServer, Handle};
use bevy_ecs::prelude::ResMut;
use bevy_ecs::system::Res;
use mcrs_protocol::Ident;
use mcrs_registry::Registry;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tracing::{debug, info};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnchantmentData {
    pub description: serde_json::Value,
    pub min_cost: EnchantmentCost,
    pub max_cost: EnchantmentCost,
    pub anvil_cost: u32,
    pub slots: Vec<String>,
    pub supported_items: String,
    pub weight: u32,
    pub max_level: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exclusive_set: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effects: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnchantmentCost {
    pub base: u32,
    pub per_level_above_first: u32,
}

fn init_enchantment_registry() -> Registry<EnchantmentData> {
    let mut registry = Registry::new();

    let silk_touch_json = include_str!("../../../../assets/minecraft/enchantment/silk_touch.json");
    let silk_touch: EnchantmentData =
        serde_json::from_str(silk_touch_json).expect("Failed to parse silk_touch.json");

    registry.insert(
        Ident::<String>::from_str("minecraft:silk_touch").unwrap(),
        silk_touch,
    );

    info!(
        count = registry.len(),
        "Enchantment registry initialized"
    );

    registry
}

/// Scans the enchantment registry for tag references (fields with '#' prefix)
/// and loads the corresponding tag files via the Bevy asset system.
fn load_enchantment_tags(
    enchantment_registry: Res<Registry<EnchantmentData>>,
    asset_server: Res<AssetServer>,
    mut tag_registry: ResMut<TagRegistry<EnchantmentData>>,
) {
    let tag_dir = "minecraft/tags/enchantment";

    for (_id, data) in enchantment_registry.iter_entries() {
        if let Some(ref tag_ref) = data.exclusive_set {
            if let Some(tag_name) = tag_ref.strip_prefix('#') {
                if let Ok(ident) = Ident::<String>::from_str(tag_name) {
                    let asset_path = format!(
                        "{}/{}.json",
                        tag_dir,
                        ident.path()
                    );
                    let dir = tag_dir.to_string();
                    debug!(
                        tag = %ident,
                        path = %asset_path,
                        "Loading enchantment tag"
                    );
                    let handle: Handle<ResourcePackTags> = asset_server.load_with_settings(
                        asset_path,
                        move |s: &mut TagFileLoaderSettings| {
                            s.directory = dir.clone();
                        },
                    );
                    tag_registry.loaded_tags.push(handle);
                }
            }
        }
    }

    info!(
        requested = tag_registry.loaded_tags.len(),
        "Requested enchantment tag files"
    );
}

pub struct EnchantmentPlugin;

impl Plugin for EnchantmentPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(init_enchantment_registry());
        app.init_resource::<TagRegistry<EnchantmentData>>();
        app.add_systems(PostStartup, load_enchantment_tags);
        app.add_systems(
            Update,
            crate::tag::block::process_loaded_tags::<EnchantmentData>,
        );
    }
}

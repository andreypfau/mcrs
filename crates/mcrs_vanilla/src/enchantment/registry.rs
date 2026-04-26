use super::data::EnchantmentData;
use bevy_asset::{AssetId, Assets, Handle};
use bevy_ecs::resource::Resource;
use mcrs_core::ResourceLocation;
use std::collections::HashMap;
use std::sync::Arc;

/// The 43 vanilla enchantments in Java bootstrap (protocol) order.
pub const VANILLA_ENCHANTMENTS: &[&str] = &[
    "minecraft:protection",
    "minecraft:fire_protection",
    "minecraft:feather_falling",
    "minecraft:blast_protection",
    "minecraft:projectile_protection",
    "minecraft:respiration",
    "minecraft:aqua_affinity",
    "minecraft:thorns",
    "minecraft:depth_strider",
    "minecraft:frost_walker",
    "minecraft:binding_curse",
    "minecraft:soul_speed",
    "minecraft:swift_sneak",
    "minecraft:sharpness",
    "minecraft:smite",
    "minecraft:bane_of_arthropods",
    "minecraft:knockback",
    "minecraft:fire_aspect",
    "minecraft:looting",
    "minecraft:sweeping_edge",
    "minecraft:efficiency",
    "minecraft:silk_touch",
    "minecraft:unbreaking",
    "minecraft:fortune",
    "minecraft:power",
    "minecraft:punch",
    "minecraft:flame",
    "minecraft:infinity",
    "minecraft:luck_of_the_sea",
    "minecraft:lure",
    "minecraft:loyalty",
    "minecraft:impaling",
    "minecraft:riptide",
    "minecraft:lunge",
    "minecraft:channeling",
    "minecraft:multishot",
    "minecraft:quick_charge",
    "minecraft:piercing",
    "minecraft:density",
    "minecraft:breach",
    "minecraft:wind_burst",
    "minecraft:mending",
    "minecraft:vanishing_curse",
];

/// Tracks loaded enchantment assets in protocol order.
///
/// `index` in `entries` == protocol_id.
#[derive(Resource)]
pub struct LoadedEnchantments {
    /// Insertion-order list: index = protocol_id
    entries: Vec<(ResourceLocation<Arc<str>>, Handle<EnchantmentData>)>,
    /// RL -> index for fast lookup
    index: HashMap<ResourceLocation<Arc<str>>, u32>,
}

impl LoadedEnchantments {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            index: HashMap::new(),
        }
    }

    pub fn push(&mut self, loc: ResourceLocation<Arc<str>>, handle: Handle<EnchantmentData>) {
        let id = self.entries.len() as u32;
        self.index.insert(loc.clone(), id);
        self.entries.push((loc, handle));
    }

    pub fn protocol_id_of(&self, loc: &str) -> Option<u32> {
        self.index.get(loc).copied()
    }

    pub fn get_handle(&self, protocol_id: u32) -> Option<&Handle<EnchantmentData>> {
        self.entries.get(protocol_id as usize).map(|(_, h)| h)
    }

    pub fn resolve_asset_id<S: AsRef<str>>(
        &self,
        loc: &ResourceLocation<S>,
        assets: &Assets<EnchantmentData>,
    ) -> Option<AssetId<EnchantmentData>> {
        let pid = self.protocol_id_of(loc.as_str())?;
        let handle = self.get_handle(pid)?;
        if assets.contains(handle.id()) {
            Some(handle.id())
        } else {
            None
        }
    }

    pub fn iter(
        &self,
    ) -> impl Iterator<Item = (u32, &ResourceLocation<Arc<str>>, &Handle<EnchantmentData>)> {
        self.entries
            .iter()
            .enumerate()
            .map(|(i, (loc, h))| (i as u32, loc, h))
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

use super::data::EnchantmentData;
use super::registry::LoadedEnchantments;
use bevy_asset::{AssetId, Assets};
use bevy_ecs::resource::Resource;
use mcrs_core::resource_location::ResourceLocation;
use mcrs_core::tag::file::{TagEntry, TagFile};
use mcrs_core::tag::key::{TagKey, TaggedRegistry};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

impl TaggedRegistry for EnchantmentData {
    const REGISTRY_PATH: &'static str = "enchantment";
}

/// Resolved enchantment tags, keyed by resource location.
///
/// Unlike `TagRegistry<T>` (which uses bitsets for static registries),
/// enchantment tags store `HashSet<AssetId<EnchantmentData>>` since
/// enchantments are dynamic Bevy assets.
#[derive(Resource, Default)]
pub struct EnchantmentTags {
    inner: HashMap<ResourceLocation<Arc<str>>, HashSet<AssetId<EnchantmentData>>>,
}

impl EnchantmentTags {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a resolved tag set.
    pub fn insert(
        &mut self,
        loc: ResourceLocation<Arc<str>>,
        ids: HashSet<AssetId<EnchantmentData>>,
    ) {
        self.inner.insert(loc, ids);
    }

    /// Check whether `id` is a member of the given tag.
    pub fn contains(&self, tag_str: &str, id: AssetId<EnchantmentData>) -> bool {
        self.inner
            .get(tag_str)
            .map_or(false, |set| set.contains(&id))
    }

    /// Return the full set of asset IDs for a tag, or `None` if not loaded.
    pub fn get(&self, tag_str: &str) -> Option<&HashSet<AssetId<EnchantmentData>>> {
        self.inner.get(tag_str)
    }

    /// Iterate over all (tag RL, id set) pairs.
    pub fn iter(
        &self,
    ) -> impl Iterator<
        Item = (
            &ResourceLocation<Arc<str>>,
            &HashSet<AssetId<EnchantmentData>>,
        ),
    > {
        self.inner.iter()
    }

    /// Recursively expand a `TagFile` into a set of `AssetId<EnchantmentData>`,
    /// resolving element references via `LoadedEnchantments`.
    pub fn resolve_tag_file(
        tag_file: &TagFile,
        all_files: &Assets<TagFile>,
        loaded: &LoadedEnchantments,
        assets: &Assets<EnchantmentData>,
    ) -> HashSet<AssetId<EnchantmentData>> {
        let mut out = HashSet::new();
        for entry in &tag_file.values {
            match entry {
                TagEntry::Element(loc) => {
                    if let Some(id) = loaded.resolve_asset_id(loc, assets) {
                        out.insert(id);
                    } else {
                        tracing::warn!("enchantment tag references unknown entry: {loc}");
                    }
                }
                TagEntry::OptionalElement(loc) => {
                    if let Some(id) = loaded.resolve_asset_id(loc, assets) {
                        out.insert(id);
                    }
                }
                TagEntry::Tag(h) | TagEntry::OptionalTag(h) => {
                    if let Some(nested) = all_files.get(h) {
                        out.extend(Self::resolve_tag_file(nested, all_files, loaded, assets));
                    }
                }
            }
        }
        out
    }
}

// ─── Top-level enchantment tags ─────────────────────────────────────────────

pub const TOOLTIP_ORDER: TagKey<EnchantmentData> =
    TagKey::new(mcrs_core::rl!("minecraft:tooltip_order"));
pub const NON_TREASURE: TagKey<EnchantmentData> =
    TagKey::new(mcrs_core::rl!("minecraft:non_treasure"));
pub const TREASURE: TagKey<EnchantmentData> = TagKey::new(mcrs_core::rl!("minecraft:treasure"));
pub const CURSE: TagKey<EnchantmentData> = TagKey::new(mcrs_core::rl!("minecraft:curse"));
pub const IN_ENCHANTING_TABLE: TagKey<EnchantmentData> =
    TagKey::new(mcrs_core::rl!("minecraft:in_enchanting_table"));
pub const TRADEABLE: TagKey<EnchantmentData> = TagKey::new(mcrs_core::rl!("minecraft:tradeable"));
pub const DOUBLE_TRADE_PRICE: TagKey<EnchantmentData> =
    TagKey::new(mcrs_core::rl!("minecraft:double_trade_price"));
pub const ON_MOB_SPAWN_EQUIPMENT: TagKey<EnchantmentData> =
    TagKey::new(mcrs_core::rl!("minecraft:on_mob_spawn_equipment"));
pub const ON_TRADED_EQUIPMENT: TagKey<EnchantmentData> =
    TagKey::new(mcrs_core::rl!("minecraft:on_traded_equipment"));
pub const ON_RANDOM_LOOT: TagKey<EnchantmentData> =
    TagKey::new(mcrs_core::rl!("minecraft:on_random_loot"));
pub const SMELTS_LOOT: TagKey<EnchantmentData> =
    TagKey::new(mcrs_core::rl!("minecraft:smelts_loot"));
pub const PREVENTS_BEE_SPAWNS_WHEN_MINING: TagKey<EnchantmentData> =
    TagKey::new(mcrs_core::rl!("minecraft:prevents_bee_spawns_when_mining"));
pub const PREVENTS_DECORATED_POT_SHATTERING: TagKey<EnchantmentData> = TagKey::new(mcrs_core::rl!(
    "minecraft:prevents_decorated_pot_shattering"
));
pub const PREVENTS_ICE_MELTING: TagKey<EnchantmentData> =
    TagKey::new(mcrs_core::rl!("minecraft:prevents_ice_melting"));
pub const PREVENTS_INFESTED_SPAWNS: TagKey<EnchantmentData> =
    TagKey::new(mcrs_core::rl!("minecraft:prevents_infested_spawns"));

// ─── exclusive_set/ enchantment tags ────────────────────────────────────────

pub const EXCLUSIVE_SET_ARMOR: TagKey<EnchantmentData> =
    TagKey::new(mcrs_core::rl!("minecraft:exclusive_set/armor"));
pub const EXCLUSIVE_SET_BOOTS: TagKey<EnchantmentData> =
    TagKey::new(mcrs_core::rl!("minecraft:exclusive_set/boots"));
pub const EXCLUSIVE_SET_BOW: TagKey<EnchantmentData> =
    TagKey::new(mcrs_core::rl!("minecraft:exclusive_set/bow"));
pub const EXCLUSIVE_SET_CROSSBOW: TagKey<EnchantmentData> =
    TagKey::new(mcrs_core::rl!("minecraft:exclusive_set/crossbow"));
pub const EXCLUSIVE_SET_DAMAGE: TagKey<EnchantmentData> =
    TagKey::new(mcrs_core::rl!("minecraft:exclusive_set/damage"));
pub const EXCLUSIVE_SET_MINING: TagKey<EnchantmentData> =
    TagKey::new(mcrs_core::rl!("minecraft:exclusive_set/mining"));
pub const EXCLUSIVE_SET_RIPTIDE: TagKey<EnchantmentData> =
    TagKey::new(mcrs_core::rl!("minecraft:exclusive_set/riptide"));

/// All enchantment tag keys, for bulk loading.
pub const ALL_ENCHANTMENT_TAGS: &[TagKey<EnchantmentData>] = &[
    TOOLTIP_ORDER,
    NON_TREASURE,
    TREASURE,
    CURSE,
    IN_ENCHANTING_TABLE,
    TRADEABLE,
    DOUBLE_TRADE_PRICE,
    ON_MOB_SPAWN_EQUIPMENT,
    ON_TRADED_EQUIPMENT,
    ON_RANDOM_LOOT,
    SMELTS_LOOT,
    PREVENTS_BEE_SPAWNS_WHEN_MINING,
    PREVENTS_DECORATED_POT_SHATTERING,
    PREVENTS_ICE_MELTING,
    PREVENTS_INFESTED_SPAWNS,
    EXCLUSIVE_SET_ARMOR,
    EXCLUSIVE_SET_BOOTS,
    EXCLUSIVE_SET_BOW,
    EXCLUSIVE_SET_CROSSBOW,
    EXCLUSIVE_SET_DAMAGE,
    EXCLUSIVE_SET_MINING,
    EXCLUSIVE_SET_RIPTIDE,
];

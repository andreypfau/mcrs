use super::data::EnchantmentData;
use mcrs_core::tag::key::{TagKey, TagRegistryType};

impl TagRegistryType for EnchantmentData {
    const REGISTRY_PATH: &'static str = "enchantment";
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

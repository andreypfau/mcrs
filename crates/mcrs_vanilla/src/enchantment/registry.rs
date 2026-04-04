use super::data::EnchantmentData;
use mcrs_core::{ResourceLocation, StaticRegistry};

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

pub fn register_all_enchantments(registry: &mut StaticRegistry<EnchantmentData>) {
    for &name in VANILLA_ENCHANTMENTS {
        let loc = ResourceLocation::parse(name).expect("invalid enchantment RL");
        let path = format!("assets/{}/enchantment/{}.json", loc.namespace(), loc.path());
        let json = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read enchantment file {path}: {e}"));
        let data: EnchantmentData = serde_json::from_str(&json)
            .unwrap_or_else(|e| panic!("failed to parse enchantment JSON {path}: {e}"));
        let leaked: &'static EnchantmentData = Box::leak(Box::new(data));
        registry.register(loc, leaked);
    }
}

use crate::item::Item;
use mcrs_core::tag::key::TagKey;

pub const SWORDS: TagKey<Item> = TagKey::new(mcrs_core::rl!("minecraft:swords"));
pub const PICKAXES: TagKey<Item> = TagKey::new(mcrs_core::rl!("minecraft:pickaxes"));
pub const AXES: TagKey<Item> = TagKey::new(mcrs_core::rl!("minecraft:axes"));
pub const SHOVELS: TagKey<Item> = TagKey::new(mcrs_core::rl!("minecraft:shovels"));
pub const HOES: TagKey<Item> = TagKey::new(mcrs_core::rl!("minecraft:hoes"));

// enchantable/* — referenced by Enchantment `supported_items` / `primary_items`.
pub const ENCHANTABLE_ARMOR: TagKey<Item> =
    TagKey::new(mcrs_core::rl!("minecraft:enchantable/armor"));
pub const ENCHANTABLE_BOW: TagKey<Item> = TagKey::new(mcrs_core::rl!("minecraft:enchantable/bow"));
pub const ENCHANTABLE_CHEST_ARMOR: TagKey<Item> =
    TagKey::new(mcrs_core::rl!("minecraft:enchantable/chest_armor"));
pub const ENCHANTABLE_CROSSBOW: TagKey<Item> =
    TagKey::new(mcrs_core::rl!("minecraft:enchantable/crossbow"));
pub const ENCHANTABLE_DURABILITY: TagKey<Item> =
    TagKey::new(mcrs_core::rl!("minecraft:enchantable/durability"));
pub const ENCHANTABLE_EQUIPPABLE: TagKey<Item> =
    TagKey::new(mcrs_core::rl!("minecraft:enchantable/equippable"));
pub const ENCHANTABLE_FIRE_ASPECT: TagKey<Item> =
    TagKey::new(mcrs_core::rl!("minecraft:enchantable/fire_aspect"));
pub const ENCHANTABLE_FISHING: TagKey<Item> =
    TagKey::new(mcrs_core::rl!("minecraft:enchantable/fishing"));
pub const ENCHANTABLE_FOOT_ARMOR: TagKey<Item> =
    TagKey::new(mcrs_core::rl!("minecraft:enchantable/foot_armor"));
pub const ENCHANTABLE_HEAD_ARMOR: TagKey<Item> =
    TagKey::new(mcrs_core::rl!("minecraft:enchantable/head_armor"));
pub const ENCHANTABLE_LEG_ARMOR: TagKey<Item> =
    TagKey::new(mcrs_core::rl!("minecraft:enchantable/leg_armor"));
pub const ENCHANTABLE_LUNGE: TagKey<Item> =
    TagKey::new(mcrs_core::rl!("minecraft:enchantable/lunge"));
pub const ENCHANTABLE_MACE: TagKey<Item> =
    TagKey::new(mcrs_core::rl!("minecraft:enchantable/mace"));
pub const ENCHANTABLE_MELEE_WEAPON: TagKey<Item> =
    TagKey::new(mcrs_core::rl!("minecraft:enchantable/melee_weapon"));
pub const ENCHANTABLE_MINING: TagKey<Item> =
    TagKey::new(mcrs_core::rl!("minecraft:enchantable/mining"));
pub const ENCHANTABLE_MINING_LOOT: TagKey<Item> =
    TagKey::new(mcrs_core::rl!("minecraft:enchantable/mining_loot"));
pub const ENCHANTABLE_SHARP_WEAPON: TagKey<Item> =
    TagKey::new(mcrs_core::rl!("minecraft:enchantable/sharp_weapon"));
pub const ENCHANTABLE_SWEEPING: TagKey<Item> =
    TagKey::new(mcrs_core::rl!("minecraft:enchantable/sweeping"));
pub const ENCHANTABLE_TRIDENT: TagKey<Item> =
    TagKey::new(mcrs_core::rl!("minecraft:enchantable/trident"));
pub const ENCHANTABLE_VANISHING: TagKey<Item> =
    TagKey::new(mcrs_core::rl!("minecraft:enchantable/vanishing"));
pub const ENCHANTABLE_WEAPON: TagKey<Item> =
    TagKey::new(mcrs_core::rl!("minecraft:enchantable/weapon"));

/// All item tag keys, for bulk loading.
pub const ALL_ITEM_TAGS: &[TagKey<Item>] = &[
    SWORDS,
    PICKAXES,
    AXES,
    SHOVELS,
    HOES,
    ENCHANTABLE_ARMOR,
    ENCHANTABLE_BOW,
    ENCHANTABLE_CHEST_ARMOR,
    ENCHANTABLE_CROSSBOW,
    ENCHANTABLE_DURABILITY,
    ENCHANTABLE_EQUIPPABLE,
    ENCHANTABLE_FIRE_ASPECT,
    ENCHANTABLE_FISHING,
    ENCHANTABLE_FOOT_ARMOR,
    ENCHANTABLE_HEAD_ARMOR,
    ENCHANTABLE_LEG_ARMOR,
    ENCHANTABLE_LUNGE,
    ENCHANTABLE_MACE,
    ENCHANTABLE_MELEE_WEAPON,
    ENCHANTABLE_MINING,
    ENCHANTABLE_MINING_LOOT,
    ENCHANTABLE_SHARP_WEAPON,
    ENCHANTABLE_SWEEPING,
    ENCHANTABLE_TRIDENT,
    ENCHANTABLE_VANISHING,
    ENCHANTABLE_WEAPON,
];

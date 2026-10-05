// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const ALLAY_DROP_ITEM_ON_BLOCK: Id<crate::TriggerType> = Id::from_static(53);
pub const ANY_BLOCK_USE: Id<crate::TriggerType> = Id::from_static(41);
pub const AVOID_VIBRATION: Id<crate::TriggerType> = Id::from_static(54);
pub const BEE_NEST_DESTROYED: Id<crate::TriggerType> = Id::from_static(37);
pub const BRED_ANIMALS: Id<crate::TriggerType> = Id::from_static(14);
pub const BREWED_POTION: Id<crate::TriggerType> = Id::from_static(10);
pub const CHANGED_DIMENSION: Id<crate::TriggerType> = Id::from_static(21);
pub const CHANNELED_LIGHTNING: Id<crate::TriggerType> = Id::from_static(30);
pub const CONSTRUCT_BEACON: Id<crate::TriggerType> = Id::from_static(11);
pub const CONSUME_ITEM: Id<crate::TriggerType> = Id::from_static(25);
pub const CRAFTER_RECIPE_CRAFTED: Id<crate::TriggerType> = Id::from_static(56);
pub const CURED_ZOMBIE_VILLAGER: Id<crate::TriggerType> = Id::from_static(17);
pub const DEFAULT_BLOCK_USE: Id<crate::TriggerType> = Id::from_static(40);
pub const EFFECTS_CHANGED: Id<crate::TriggerType> = Id::from_static(26);
pub const ENCHANTED_ITEM: Id<crate::TriggerType> = Id::from_static(8);
pub const ENTER_BLOCK: Id<crate::TriggerType> = Id::from_static(3);
pub const ENTITY_HURT_PLAYER: Id<crate::TriggerType> = Id::from_static(7);
pub const ENTITY_KILLED_PLAYER: Id<crate::TriggerType> = Id::from_static(2);
pub const FALL_AFTER_EXPLOSION: Id<crate::TriggerType> = Id::from_static(57);
pub const FALL_FROM_HEIGHT: Id<crate::TriggerType> = Id::from_static(50);
pub const FILLED_BUCKET: Id<crate::TriggerType> = Id::from_static(9);
pub const FISHING_ROD_HOOKED: Id<crate::TriggerType> = Id::from_static(29);
pub const HERO_OF_THE_VILLAGE: Id<crate::TriggerType> = Id::from_static(34);
pub const IMPOSSIBLE: Id<crate::TriggerType> = Id::from_static(0);
pub const INVENTORY_CHANGED: Id<crate::TriggerType> = Id::from_static(4);
pub const ITEM_DURABILITY_CHANGED: Id<crate::TriggerType> = Id::from_static(19);
pub const ITEM_USED_ON_BLOCK: Id<crate::TriggerType> = Id::from_static(39);
pub const KILL_MOB_NEAR_SCULK_CATALYST: Id<crate::TriggerType> = Id::from_static(52);
pub const KILLED_BY_ARROW: Id<crate::TriggerType> = Id::from_static(33);
pub const LEVITATION: Id<crate::TriggerType> = Id::from_static(20);
pub const LIGHTNING_STRIKE: Id<crate::TriggerType> = Id::from_static(48);
pub const LOCATION: Id<crate::TriggerType> = Id::from_static(15);
pub const NETHER_TRAVEL: Id<crate::TriggerType> = Id::from_static(28);
pub const PLACED_BLOCK: Id<crate::TriggerType> = Id::from_static(24);
pub const PLAYER_GENERATES_CONTAINER_LOOT: Id<crate::TriggerType> = Id::from_static(42);
pub const PLAYER_HURT_ENTITY: Id<crate::TriggerType> = Id::from_static(6);
pub const PLAYER_INTERACTED_WITH_ENTITY: Id<crate::TriggerType> = Id::from_static(45);
pub const PLAYER_KILLED_ENTITY: Id<crate::TriggerType> = Id::from_static(1);
pub const PLAYER_SHEARED_EQUIPMENT: Id<crate::TriggerType> = Id::from_static(46);
pub const RECIPE_CRAFTED: Id<crate::TriggerType> = Id::from_static(55);
pub const RECIPE_UNLOCKED: Id<crate::TriggerType> = Id::from_static(5);
pub const RIDE_ENTITY_IN_LAVA: Id<crate::TriggerType> = Id::from_static(51);
pub const SHOT_CROSSBOW: Id<crate::TriggerType> = Id::from_static(31);
pub const SLEPT_IN_BED: Id<crate::TriggerType> = Id::from_static(16);
pub const SLIDE_DOWN_BLOCK: Id<crate::TriggerType> = Id::from_static(36);
pub const SPEAR_MOBS: Id<crate::TriggerType> = Id::from_static(32);
pub const STARTED_RIDING: Id<crate::TriggerType> = Id::from_static(47);
pub const SUMMONED_ENTITY: Id<crate::TriggerType> = Id::from_static(13);
pub const TAME_ANIMAL: Id<crate::TriggerType> = Id::from_static(23);
pub const TARGET_HIT: Id<crate::TriggerType> = Id::from_static(38);
pub const THROWN_ITEM_PICKED_UP_BY_ENTITY: Id<crate::TriggerType> = Id::from_static(43);
pub const THROWN_ITEM_PICKED_UP_BY_PLAYER: Id<crate::TriggerType> = Id::from_static(44);
pub const TICK: Id<crate::TriggerType> = Id::from_static(22);
pub const USED_ENDER_EYE: Id<crate::TriggerType> = Id::from_static(12);
pub const USED_TOTEM: Id<crate::TriggerType> = Id::from_static(27);
pub const USING_ITEM: Id<crate::TriggerType> = Id::from_static(49);
pub const VILLAGER_TRADE: Id<crate::TriggerType> = Id::from_static(18);
pub const VOLUNTARY_EXILE: Id<crate::TriggerType> = Id::from_static(35);

pub const NAMES: &[&str] = &[
    "minecraft:impossible",
    "minecraft:player_killed_entity",
    "minecraft:entity_killed_player",
    "minecraft:enter_block",
    "minecraft:inventory_changed",
    "minecraft:recipe_unlocked",
    "minecraft:player_hurt_entity",
    "minecraft:entity_hurt_player",
    "minecraft:enchanted_item",
    "minecraft:filled_bucket",
    "minecraft:brewed_potion",
    "minecraft:construct_beacon",
    "minecraft:used_ender_eye",
    "minecraft:summoned_entity",
    "minecraft:bred_animals",
    "minecraft:location",
    "minecraft:slept_in_bed",
    "minecraft:cured_zombie_villager",
    "minecraft:villager_trade",
    "minecraft:item_durability_changed",
    "minecraft:levitation",
    "minecraft:changed_dimension",
    "minecraft:tick",
    "minecraft:tame_animal",
    "minecraft:placed_block",
    "minecraft:consume_item",
    "minecraft:effects_changed",
    "minecraft:used_totem",
    "minecraft:nether_travel",
    "minecraft:fishing_rod_hooked",
    "minecraft:channeled_lightning",
    "minecraft:shot_crossbow",
    "minecraft:spear_mobs",
    "minecraft:killed_by_arrow",
    "minecraft:hero_of_the_village",
    "minecraft:voluntary_exile",
    "minecraft:slide_down_block",
    "minecraft:bee_nest_destroyed",
    "minecraft:target_hit",
    "minecraft:item_used_on_block",
    "minecraft:default_block_use",
    "minecraft:any_block_use",
    "minecraft:player_generates_container_loot",
    "minecraft:thrown_item_picked_up_by_entity",
    "minecraft:thrown_item_picked_up_by_player",
    "minecraft:player_interacted_with_entity",
    "minecraft:player_sheared_equipment",
    "minecraft:started_riding",
    "minecraft:lightning_strike",
    "minecraft:using_item",
    "minecraft:fall_from_height",
    "minecraft:ride_entity_in_lava",
    "minecraft:kill_mob_near_sculk_catalyst",
    "minecraft:allay_drop_item_on_block",
    "minecraft:avoid_vibration",
    "minecraft:recipe_crafted",
    "minecraft:crafter_recipe_crafted",
    "minecraft:fall_after_explosion",
];

// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_core::{StaticResourceLocation, rl};

pub const IMPOSSIBLE: StaticResourceLocation = rl!("minecraft:impossible");
pub const PLAYER_KILLED_ENTITY: StaticResourceLocation = rl!("minecraft:player_killed_entity");
pub const ENTITY_KILLED_PLAYER: StaticResourceLocation = rl!("minecraft:entity_killed_player");
pub const ENTER_BLOCK: StaticResourceLocation = rl!("minecraft:enter_block");
pub const INVENTORY_CHANGED: StaticResourceLocation = rl!("minecraft:inventory_changed");
pub const RECIPE_UNLOCKED: StaticResourceLocation = rl!("minecraft:recipe_unlocked");
pub const PLAYER_HURT_ENTITY: StaticResourceLocation = rl!("minecraft:player_hurt_entity");
pub const ENTITY_HURT_PLAYER: StaticResourceLocation = rl!("minecraft:entity_hurt_player");
pub const ENCHANTED_ITEM: StaticResourceLocation = rl!("minecraft:enchanted_item");
pub const FILLED_BUCKET: StaticResourceLocation = rl!("minecraft:filled_bucket");
pub const BREWED_POTION: StaticResourceLocation = rl!("minecraft:brewed_potion");
pub const CONSTRUCT_BEACON: StaticResourceLocation = rl!("minecraft:construct_beacon");
pub const USED_ENDER_EYE: StaticResourceLocation = rl!("minecraft:used_ender_eye");
pub const SUMMONED_ENTITY: StaticResourceLocation = rl!("minecraft:summoned_entity");
pub const BRED_ANIMALS: StaticResourceLocation = rl!("minecraft:bred_animals");
pub const LOCATION: StaticResourceLocation = rl!("minecraft:location");
pub const SLEPT_IN_BED: StaticResourceLocation = rl!("minecraft:slept_in_bed");
pub const CURED_ZOMBIE_VILLAGER: StaticResourceLocation = rl!("minecraft:cured_zombie_villager");
pub const VILLAGER_TRADE: StaticResourceLocation = rl!("minecraft:villager_trade");
pub const ITEM_DURABILITY_CHANGED: StaticResourceLocation = rl!("minecraft:item_durability_changed");
pub const LEVITATION: StaticResourceLocation = rl!("minecraft:levitation");
pub const CHANGED_DIMENSION: StaticResourceLocation = rl!("minecraft:changed_dimension");
pub const TICK: StaticResourceLocation = rl!("minecraft:tick");
pub const TAME_ANIMAL: StaticResourceLocation = rl!("minecraft:tame_animal");
pub const PLACED_BLOCK: StaticResourceLocation = rl!("minecraft:placed_block");
pub const CONSUME_ITEM: StaticResourceLocation = rl!("minecraft:consume_item");
pub const EFFECTS_CHANGED: StaticResourceLocation = rl!("minecraft:effects_changed");
pub const USED_TOTEM: StaticResourceLocation = rl!("minecraft:used_totem");
pub const NETHER_TRAVEL: StaticResourceLocation = rl!("minecraft:nether_travel");
pub const FISHING_ROD_HOOKED: StaticResourceLocation = rl!("minecraft:fishing_rod_hooked");
pub const CHANNELED_LIGHTNING: StaticResourceLocation = rl!("minecraft:channeled_lightning");
pub const SHOT_CROSSBOW: StaticResourceLocation = rl!("minecraft:shot_crossbow");
pub const SPEAR_MOBS: StaticResourceLocation = rl!("minecraft:spear_mobs");
pub const KILLED_BY_ARROW: StaticResourceLocation = rl!("minecraft:killed_by_arrow");
pub const HERO_OF_THE_VILLAGE: StaticResourceLocation = rl!("minecraft:hero_of_the_village");
pub const VOLUNTARY_EXILE: StaticResourceLocation = rl!("minecraft:voluntary_exile");
pub const SLIDE_DOWN_BLOCK: StaticResourceLocation = rl!("minecraft:slide_down_block");
pub const BEE_NEST_DESTROYED: StaticResourceLocation = rl!("minecraft:bee_nest_destroyed");
pub const TARGET_HIT: StaticResourceLocation = rl!("minecraft:target_hit");
pub const ITEM_USED_ON_BLOCK: StaticResourceLocation = rl!("minecraft:item_used_on_block");
pub const DEFAULT_BLOCK_USE: StaticResourceLocation = rl!("minecraft:default_block_use");
pub const ANY_BLOCK_USE: StaticResourceLocation = rl!("minecraft:any_block_use");
pub const PLAYER_GENERATES_CONTAINER_LOOT: StaticResourceLocation = rl!("minecraft:player_generates_container_loot");
pub const THROWN_ITEM_PICKED_UP_BY_ENTITY: StaticResourceLocation = rl!("minecraft:thrown_item_picked_up_by_entity");
pub const THROWN_ITEM_PICKED_UP_BY_PLAYER: StaticResourceLocation = rl!("minecraft:thrown_item_picked_up_by_player");
pub const PLAYER_INTERACTED_WITH_ENTITY: StaticResourceLocation = rl!("minecraft:player_interacted_with_entity");
pub const PLAYER_SHEARED_EQUIPMENT: StaticResourceLocation = rl!("minecraft:player_sheared_equipment");
pub const STARTED_RIDING: StaticResourceLocation = rl!("minecraft:started_riding");
pub const LIGHTNING_STRIKE: StaticResourceLocation = rl!("minecraft:lightning_strike");
pub const USING_ITEM: StaticResourceLocation = rl!("minecraft:using_item");
pub const FALL_FROM_HEIGHT: StaticResourceLocation = rl!("minecraft:fall_from_height");
pub const RIDE_ENTITY_IN_LAVA: StaticResourceLocation = rl!("minecraft:ride_entity_in_lava");
pub const KILL_MOB_NEAR_SCULK_CATALYST: StaticResourceLocation = rl!("minecraft:kill_mob_near_sculk_catalyst");
pub const ALLAY_DROP_ITEM_ON_BLOCK: StaticResourceLocation = rl!("minecraft:allay_drop_item_on_block");
pub const AVOID_VIBRATION: StaticResourceLocation = rl!("minecraft:avoid_vibration");
pub const RECIPE_CRAFTED: StaticResourceLocation = rl!("minecraft:recipe_crafted");
pub const CRAFTER_RECIPE_CRAFTED: StaticResourceLocation = rl!("minecraft:crafter_recipe_crafted");
pub const FALL_AFTER_EXPLOSION: StaticResourceLocation = rl!("minecraft:fall_after_explosion");

pub const ENTRIES: &[StaticResourceLocation] = &[
    IMPOSSIBLE,
    PLAYER_KILLED_ENTITY,
    ENTITY_KILLED_PLAYER,
    ENTER_BLOCK,
    INVENTORY_CHANGED,
    RECIPE_UNLOCKED,
    PLAYER_HURT_ENTITY,
    ENTITY_HURT_PLAYER,
    ENCHANTED_ITEM,
    FILLED_BUCKET,
    BREWED_POTION,
    CONSTRUCT_BEACON,
    USED_ENDER_EYE,
    SUMMONED_ENTITY,
    BRED_ANIMALS,
    LOCATION,
    SLEPT_IN_BED,
    CURED_ZOMBIE_VILLAGER,
    VILLAGER_TRADE,
    ITEM_DURABILITY_CHANGED,
    LEVITATION,
    CHANGED_DIMENSION,
    TICK,
    TAME_ANIMAL,
    PLACED_BLOCK,
    CONSUME_ITEM,
    EFFECTS_CHANGED,
    USED_TOTEM,
    NETHER_TRAVEL,
    FISHING_ROD_HOOKED,
    CHANNELED_LIGHTNING,
    SHOT_CROSSBOW,
    SPEAR_MOBS,
    KILLED_BY_ARROW,
    HERO_OF_THE_VILLAGE,
    VOLUNTARY_EXILE,
    SLIDE_DOWN_BLOCK,
    BEE_NEST_DESTROYED,
    TARGET_HIT,
    ITEM_USED_ON_BLOCK,
    DEFAULT_BLOCK_USE,
    ANY_BLOCK_USE,
    PLAYER_GENERATES_CONTAINER_LOOT,
    THROWN_ITEM_PICKED_UP_BY_ENTITY,
    THROWN_ITEM_PICKED_UP_BY_PLAYER,
    PLAYER_INTERACTED_WITH_ENTITY,
    PLAYER_SHEARED_EQUIPMENT,
    STARTED_RIDING,
    LIGHTNING_STRIKE,
    USING_ITEM,
    FALL_FROM_HEIGHT,
    RIDE_ENTITY_IN_LAVA,
    KILL_MOB_NEAR_SCULK_CATALYST,
    ALLAY_DROP_ITEM_ON_BLOCK,
    AVOID_VIBRATION,
    RECIPE_CRAFTED,
    CRAFTER_RECIPE_CRAFTED,
    FALL_AFTER_EXPLOSION,
];

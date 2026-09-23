use mcrs_minecraft_core::rl;
use mcrs_minecraft_registry::StaticRegistry;

use super::EntityType;

pub static ACACIA_BOAT: EntityType = EntityType::new(rl!("minecraft:acacia_boat"), 0);
pub static ACACIA_CHEST_BOAT: EntityType = EntityType::new(rl!("minecraft:acacia_chest_boat"), 1);
pub static ALLAY: EntityType = EntityType::new(rl!("minecraft:allay"), 2);
pub static AREA_EFFECT_CLOUD: EntityType = EntityType::new(rl!("minecraft:area_effect_cloud"), 3);
pub static ARMADILLO: EntityType = EntityType::new(rl!("minecraft:armadillo"), 4);
pub static ARMOR_STAND: EntityType = EntityType::new(rl!("minecraft:armor_stand"), 5);
pub static ARROW: EntityType = EntityType::new(rl!("minecraft:arrow"), 6);
pub static AXOLOTL: EntityType = EntityType::new(rl!("minecraft:axolotl"), 7);
pub static BAMBOO_CHEST_RAFT: EntityType = EntityType::new(rl!("minecraft:bamboo_chest_raft"), 8);
pub static BAMBOO_RAFT: EntityType = EntityType::new(rl!("minecraft:bamboo_raft"), 9);
pub static BAT: EntityType = EntityType::new(rl!("minecraft:bat"), 10);
pub static BEE: EntityType = EntityType::new(rl!("minecraft:bee"), 11);
pub static BIRCH_BOAT: EntityType = EntityType::new(rl!("minecraft:birch_boat"), 12);
pub static BIRCH_CHEST_BOAT: EntityType = EntityType::new(rl!("minecraft:birch_chest_boat"), 13);
pub static BLAZE: EntityType = EntityType::new(rl!("minecraft:blaze"), 14);
pub static BLOCK_DISPLAY: EntityType = EntityType::new(rl!("minecraft:block_display"), 15);
pub static BOGGED: EntityType = EntityType::new(rl!("minecraft:bogged"), 16);
pub static BREEZE: EntityType = EntityType::new(rl!("minecraft:breeze"), 17);
pub static BREEZE_WIND_CHARGE: EntityType =
    EntityType::new(rl!("minecraft:breeze_wind_charge"), 18);
pub static CAMEL: EntityType = EntityType::new(rl!("minecraft:camel"), 19);
pub static CAMEL_HUSK: EntityType = EntityType::new(rl!("minecraft:camel_husk"), 20);
pub static CAT: EntityType = EntityType::new(rl!("minecraft:cat"), 21);
pub static CAVE_SPIDER: EntityType = EntityType::new(rl!("minecraft:cave_spider"), 22);
pub static CHERRY_BOAT: EntityType = EntityType::new(rl!("minecraft:cherry_boat"), 23);
pub static CHERRY_CHEST_BOAT: EntityType = EntityType::new(rl!("minecraft:cherry_chest_boat"), 24);
pub static CHEST_MINECART: EntityType = EntityType::new(rl!("minecraft:chest_minecart"), 25);
pub static CHICKEN: EntityType = EntityType::new(rl!("minecraft:chicken"), 26);
pub static COD: EntityType = EntityType::new(rl!("minecraft:cod"), 27);
pub static COPPER_GOLEM: EntityType = EntityType::new(rl!("minecraft:copper_golem"), 28);
pub static COMMAND_BLOCK_MINECART: EntityType =
    EntityType::new(rl!("minecraft:command_block_minecart"), 29);
pub static COW: EntityType = EntityType::new(rl!("minecraft:cow"), 30);
pub static CREAKING: EntityType = EntityType::new(rl!("minecraft:creaking"), 31);
pub static CREEPER: EntityType = EntityType::new(rl!("minecraft:creeper"), 32);
pub static CUSHION: EntityType = EntityType::new(rl!("minecraft:cushion"), 33);
pub static DARK_OAK_BOAT: EntityType = EntityType::new(rl!("minecraft:dark_oak_boat"), 34);
pub static DARK_OAK_CHEST_BOAT: EntityType =
    EntityType::new(rl!("minecraft:dark_oak_chest_boat"), 35);
pub static DOLPHIN: EntityType = EntityType::new(rl!("minecraft:dolphin"), 36);
pub static DONKEY: EntityType = EntityType::new(rl!("minecraft:donkey"), 37);
pub static DRAGON_FIREBALL: EntityType = EntityType::new(rl!("minecraft:dragon_fireball"), 38);
pub static DROWNED: EntityType = EntityType::new(rl!("minecraft:drowned"), 39);
pub static EGG: EntityType = EntityType::new(rl!("minecraft:egg"), 40);
pub static ELDER_GUARDIAN: EntityType = EntityType::new(rl!("minecraft:elder_guardian"), 41);
pub static ENDERMAN: EntityType = EntityType::new(rl!("minecraft:enderman"), 42);
pub static ENDERMITE: EntityType = EntityType::new(rl!("minecraft:endermite"), 43);
pub static ENDER_DRAGON: EntityType = EntityType::new(rl!("minecraft:ender_dragon"), 44);
pub static ENDER_PEARL: EntityType = EntityType::new(rl!("minecraft:ender_pearl"), 45);
pub static END_CRYSTAL: EntityType = EntityType::new(rl!("minecraft:end_crystal"), 46);
pub static EVOKER: EntityType = EntityType::new(rl!("minecraft:evoker"), 47);
pub static EVOKER_FANGS: EntityType = EntityType::new(rl!("minecraft:evoker_fangs"), 48);
pub static EXPERIENCE_BOTTLE: EntityType = EntityType::new(rl!("minecraft:experience_bottle"), 49);
pub static EXPERIENCE_ORB: EntityType = EntityType::new(rl!("minecraft:experience_orb"), 50);
pub static EYE_OF_ENDER: EntityType = EntityType::new(rl!("minecraft:eye_of_ender"), 51);
pub static FALLING_BLOCK: EntityType = EntityType::new(rl!("minecraft:falling_block"), 52);
pub static FIREBALL: EntityType = EntityType::new(rl!("minecraft:fireball"), 53);
pub static FIREWORK_ROCKET: EntityType = EntityType::new(rl!("minecraft:firework_rocket"), 54);
pub static FOX: EntityType = EntityType::new(rl!("minecraft:fox"), 55);
pub static FROG: EntityType = EntityType::new(rl!("minecraft:frog"), 56);
pub static FURNACE_MINECART: EntityType = EntityType::new(rl!("minecraft:furnace_minecart"), 57);
pub static GHAST: EntityType = EntityType::new(rl!("minecraft:ghast"), 58);
pub static HAPPY_GHAST: EntityType = EntityType::new(rl!("minecraft:happy_ghast"), 59);
pub static GIANT: EntityType = EntityType::new(rl!("minecraft:giant"), 60);
pub static GLOW_ITEM_FRAME: EntityType = EntityType::new(rl!("minecraft:glow_item_frame"), 61);
pub static GLOW_SQUID: EntityType = EntityType::new(rl!("minecraft:glow_squid"), 62);
pub static GOAT: EntityType = EntityType::new(rl!("minecraft:goat"), 63);
pub static GUARDIAN: EntityType = EntityType::new(rl!("minecraft:guardian"), 64);
pub static HOGLIN: EntityType = EntityType::new(rl!("minecraft:hoglin"), 65);
pub static HOPPER_MINECART: EntityType = EntityType::new(rl!("minecraft:hopper_minecart"), 66);
pub static HORSE: EntityType = EntityType::new(rl!("minecraft:horse"), 67);
pub static HUSK: EntityType = EntityType::new(rl!("minecraft:husk"), 68);
pub static ILLUSIONER: EntityType = EntityType::new(rl!("minecraft:illusioner"), 69);
pub static INTERACTION: EntityType = EntityType::new(rl!("minecraft:interaction"), 70);
pub static IRON_GOLEM: EntityType = EntityType::new(rl!("minecraft:iron_golem"), 71);
pub static ITEM: EntityType = EntityType::new(rl!("minecraft:item"), 72);
pub static ITEM_DISPLAY: EntityType = EntityType::new(rl!("minecraft:item_display"), 73);
pub static ITEM_FRAME: EntityType = EntityType::new(rl!("minecraft:item_frame"), 74);
pub static JUNGLE_BOAT: EntityType = EntityType::new(rl!("minecraft:jungle_boat"), 75);
pub static JUNGLE_CHEST_BOAT: EntityType = EntityType::new(rl!("minecraft:jungle_chest_boat"), 76);
pub static LEASH_KNOT: EntityType = EntityType::new(rl!("minecraft:leash_knot"), 77);
pub static LIGHTNING_BOLT: EntityType = EntityType::new(rl!("minecraft:lightning_bolt"), 78);
pub static LLAMA: EntityType = EntityType::new(rl!("minecraft:llama"), 79);
pub static LLAMA_SPIT: EntityType = EntityType::new(rl!("minecraft:llama_spit"), 80);
pub static MAGMA_CUBE: EntityType = EntityType::new(rl!("minecraft:magma_cube"), 81);
pub static MANGROVE_BOAT: EntityType = EntityType::new(rl!("minecraft:mangrove_boat"), 82);
pub static MANGROVE_CHEST_BOAT: EntityType =
    EntityType::new(rl!("minecraft:mangrove_chest_boat"), 83);
pub static MANNEQUIN: EntityType = EntityType::new(rl!("minecraft:mannequin"), 84);
pub static MARKER: EntityType = EntityType::new(rl!("minecraft:marker"), 85);
pub static MINECART: EntityType = EntityType::new(rl!("minecraft:minecart"), 86);
pub static MOOSHROOM: EntityType = EntityType::new(rl!("minecraft:mooshroom"), 87);
pub static MULE: EntityType = EntityType::new(rl!("minecraft:mule"), 88);
pub static NAUTILUS: EntityType = EntityType::new(rl!("minecraft:nautilus"), 89);
pub static OAK_BOAT: EntityType = EntityType::new(rl!("minecraft:oak_boat"), 90);
pub static OAK_CHEST_BOAT: EntityType = EntityType::new(rl!("minecraft:oak_chest_boat"), 91);
pub static OCELOT: EntityType = EntityType::new(rl!("minecraft:ocelot"), 92);
pub static OMINOUS_ITEM_SPAWNER: EntityType =
    EntityType::new(rl!("minecraft:ominous_item_spawner"), 93);
pub static PAINTING: EntityType = EntityType::new(rl!("minecraft:painting"), 94);
pub static PALE_OAK_BOAT: EntityType = EntityType::new(rl!("minecraft:pale_oak_boat"), 95);
pub static PALE_OAK_CHEST_BOAT: EntityType =
    EntityType::new(rl!("minecraft:pale_oak_chest_boat"), 96);
pub static PANDA: EntityType = EntityType::new(rl!("minecraft:panda"), 97);
pub static PARCHED: EntityType = EntityType::new(rl!("minecraft:parched"), 98);
pub static PARROT: EntityType = EntityType::new(rl!("minecraft:parrot"), 99);
pub static PHANTOM: EntityType = EntityType::new(rl!("minecraft:phantom"), 100);
pub static PIG: EntityType = EntityType::new(rl!("minecraft:pig"), 101);
pub static PIGLIN: EntityType = EntityType::new(rl!("minecraft:piglin"), 102);
pub static PIGLIN_BRUTE: EntityType = EntityType::new(rl!("minecraft:piglin_brute"), 103);
pub static PILLAGER: EntityType = EntityType::new(rl!("minecraft:pillager"), 104);
pub static POLAR_BEAR: EntityType = EntityType::new(rl!("minecraft:polar_bear"), 105);
pub static POPLAR_BOAT: EntityType = EntityType::new(rl!("minecraft:poplar_boat"), 106);
pub static POPLAR_CHEST_BOAT: EntityType = EntityType::new(rl!("minecraft:poplar_chest_boat"), 107);
pub static SPLASH_POTION: EntityType = EntityType::new(rl!("minecraft:splash_potion"), 108);
pub static LINGERING_POTION: EntityType = EntityType::new(rl!("minecraft:lingering_potion"), 109);
pub static PUFFERFISH: EntityType = EntityType::new(rl!("minecraft:pufferfish"), 110);
pub static RABBIT: EntityType = EntityType::new(rl!("minecraft:rabbit"), 111);
pub static RAVAGER: EntityType = EntityType::new(rl!("minecraft:ravager"), 112);
pub static SALMON: EntityType = EntityType::new(rl!("minecraft:salmon"), 113);
pub static SHEEP: EntityType = EntityType::new(rl!("minecraft:sheep"), 114);
pub static SHULKER: EntityType = EntityType::new(rl!("minecraft:shulker"), 115);
pub static SHULKER_BULLET: EntityType = EntityType::new(rl!("minecraft:shulker_bullet"), 116);
pub static SILVERFISH: EntityType = EntityType::new(rl!("minecraft:silverfish"), 117);
pub static SKELETON: EntityType = EntityType::new(rl!("minecraft:skeleton"), 118);
pub static SKELETON_HORSE: EntityType = EntityType::new(rl!("minecraft:skeleton_horse"), 119);
pub static SLIME: EntityType = EntityType::new(rl!("minecraft:slime"), 120);
pub static SMALL_FIREBALL: EntityType = EntityType::new(rl!("minecraft:small_fireball"), 121);
pub static SNIFFER: EntityType = EntityType::new(rl!("minecraft:sniffer"), 122);
pub static SNOWBALL: EntityType = EntityType::new(rl!("minecraft:snowball"), 123);
pub static SNOW_GOLEM: EntityType = EntityType::new(rl!("minecraft:snow_golem"), 124);
pub static SPAWNER_MINECART: EntityType = EntityType::new(rl!("minecraft:spawner_minecart"), 125);
pub static SPECTRAL_ARROW: EntityType = EntityType::new(rl!("minecraft:spectral_arrow"), 126);
pub static SPIDER: EntityType = EntityType::new(rl!("minecraft:spider"), 127);
pub static SPRUCE_BOAT: EntityType = EntityType::new(rl!("minecraft:spruce_boat"), 128);
pub static SPRUCE_CHEST_BOAT: EntityType = EntityType::new(rl!("minecraft:spruce_chest_boat"), 129);
pub static SQUID: EntityType = EntityType::new(rl!("minecraft:squid"), 130);
pub static STRAY: EntityType = EntityType::new(rl!("minecraft:stray"), 131);
pub static STRIDER: EntityType = EntityType::new(rl!("minecraft:strider"), 132);
pub static SULFUR_CUBE: EntityType = EntityType::new(rl!("minecraft:sulfur_cube"), 133);
pub static TADPOLE: EntityType = EntityType::new(rl!("minecraft:tadpole"), 134);
pub static TEXT_DISPLAY: EntityType = EntityType::new(rl!("minecraft:text_display"), 135);
pub static PRIMED_TNT: EntityType = EntityType::new(rl!("minecraft:tnt"), 136);
pub static TNT_MINECART: EntityType = EntityType::new(rl!("minecraft:tnt_minecart"), 137);
pub static TRADER_LLAMA: EntityType = EntityType::new(rl!("minecraft:trader_llama"), 138);
pub static TRIDENT: EntityType = EntityType::new(rl!("minecraft:trident"), 139);
pub static TROPICAL_FISH: EntityType = EntityType::new(rl!("minecraft:tropical_fish"), 140);
pub static TURTLE: EntityType = EntityType::new(rl!("minecraft:turtle"), 141);
pub static VEX: EntityType = EntityType::new(rl!("minecraft:vex"), 142);
pub static VILLAGER: EntityType = EntityType::new(rl!("minecraft:villager"), 143);
pub static VINDICATOR: EntityType = EntityType::new(rl!("minecraft:vindicator"), 144);
pub static WANDERING_TRADER: EntityType = EntityType::new(rl!("minecraft:wandering_trader"), 145);
pub static WARDEN: EntityType = EntityType::new(rl!("minecraft:warden"), 146);
pub static WIND_CHARGE: EntityType = EntityType::new(rl!("minecraft:wind_charge"), 147);
pub static WITCH: EntityType = EntityType::new(rl!("minecraft:witch"), 148);
pub static WITHER: EntityType = EntityType::new(rl!("minecraft:wither"), 149);
pub static WITHER_SKELETON: EntityType = EntityType::new(rl!("minecraft:wither_skeleton"), 150);
pub static WITHER_SKULL: EntityType = EntityType::new(rl!("minecraft:wither_skull"), 151);
pub static WOLF: EntityType = EntityType::new(rl!("minecraft:wolf"), 152);
pub static ZOGLIN: EntityType = EntityType::new(rl!("minecraft:zoglin"), 153);
pub static ZOMBIE: EntityType = EntityType::new(rl!("minecraft:zombie"), 154);
pub static ZOMBIE_HORSE: EntityType = EntityType::new(rl!("minecraft:zombie_horse"), 155);
pub static ZOMBIE_NAUTILUS: EntityType = EntityType::new(rl!("minecraft:zombie_nautilus"), 156);
pub static ZOMBIE_VILLAGER: EntityType = EntityType::new(rl!("minecraft:zombie_villager"), 157);
pub static ZOMBIFIED_PIGLIN: EntityType = EntityType::new(rl!("minecraft:zombified_piglin"), 158);
pub static PLAYER: EntityType = EntityType::new(rl!("minecraft:player"), 159);
pub static FISHING_BOBBER: EntityType = EntityType::new(rl!("minecraft:fishing_bobber"), 160);

pub static ALL: [&EntityType; 161] = [
    &ACACIA_BOAT,
    &ACACIA_CHEST_BOAT,
    &ALLAY,
    &AREA_EFFECT_CLOUD,
    &ARMADILLO,
    &ARMOR_STAND,
    &ARROW,
    &AXOLOTL,
    &BAMBOO_CHEST_RAFT,
    &BAMBOO_RAFT,
    &BAT,
    &BEE,
    &BIRCH_BOAT,
    &BIRCH_CHEST_BOAT,
    &BLAZE,
    &BLOCK_DISPLAY,
    &BOGGED,
    &BREEZE,
    &BREEZE_WIND_CHARGE,
    &CAMEL,
    &CAMEL_HUSK,
    &CAT,
    &CAVE_SPIDER,
    &CHERRY_BOAT,
    &CHERRY_CHEST_BOAT,
    &CHEST_MINECART,
    &CHICKEN,
    &COD,
    &COPPER_GOLEM,
    &COMMAND_BLOCK_MINECART,
    &COW,
    &CREAKING,
    &CREEPER,
    &CUSHION,
    &DARK_OAK_BOAT,
    &DARK_OAK_CHEST_BOAT,
    &DOLPHIN,
    &DONKEY,
    &DRAGON_FIREBALL,
    &DROWNED,
    &EGG,
    &ELDER_GUARDIAN,
    &ENDERMAN,
    &ENDERMITE,
    &ENDER_DRAGON,
    &ENDER_PEARL,
    &END_CRYSTAL,
    &EVOKER,
    &EVOKER_FANGS,
    &EXPERIENCE_BOTTLE,
    &EXPERIENCE_ORB,
    &EYE_OF_ENDER,
    &FALLING_BLOCK,
    &FIREBALL,
    &FIREWORK_ROCKET,
    &FOX,
    &FROG,
    &FURNACE_MINECART,
    &GHAST,
    &HAPPY_GHAST,
    &GIANT,
    &GLOW_ITEM_FRAME,
    &GLOW_SQUID,
    &GOAT,
    &GUARDIAN,
    &HOGLIN,
    &HOPPER_MINECART,
    &HORSE,
    &HUSK,
    &ILLUSIONER,
    &INTERACTION,
    &IRON_GOLEM,
    &ITEM,
    &ITEM_DISPLAY,
    &ITEM_FRAME,
    &JUNGLE_BOAT,
    &JUNGLE_CHEST_BOAT,
    &LEASH_KNOT,
    &LIGHTNING_BOLT,
    &LLAMA,
    &LLAMA_SPIT,
    &MAGMA_CUBE,
    &MANGROVE_BOAT,
    &MANGROVE_CHEST_BOAT,
    &MANNEQUIN,
    &MARKER,
    &MINECART,
    &MOOSHROOM,
    &MULE,
    &NAUTILUS,
    &OAK_BOAT,
    &OAK_CHEST_BOAT,
    &OCELOT,
    &OMINOUS_ITEM_SPAWNER,
    &PAINTING,
    &PALE_OAK_BOAT,
    &PALE_OAK_CHEST_BOAT,
    &PANDA,
    &PARCHED,
    &PARROT,
    &PHANTOM,
    &PIG,
    &PIGLIN,
    &PIGLIN_BRUTE,
    &PILLAGER,
    &POLAR_BEAR,
    &POPLAR_BOAT,
    &POPLAR_CHEST_BOAT,
    &SPLASH_POTION,
    &LINGERING_POTION,
    &PUFFERFISH,
    &RABBIT,
    &RAVAGER,
    &SALMON,
    &SHEEP,
    &SHULKER,
    &SHULKER_BULLET,
    &SILVERFISH,
    &SKELETON,
    &SKELETON_HORSE,
    &SLIME,
    &SMALL_FIREBALL,
    &SNIFFER,
    &SNOWBALL,
    &SNOW_GOLEM,
    &SPAWNER_MINECART,
    &SPECTRAL_ARROW,
    &SPIDER,
    &SPRUCE_BOAT,
    &SPRUCE_CHEST_BOAT,
    &SQUID,
    &STRAY,
    &STRIDER,
    &SULFUR_CUBE,
    &TADPOLE,
    &TEXT_DISPLAY,
    &PRIMED_TNT,
    &TNT_MINECART,
    &TRADER_LLAMA,
    &TRIDENT,
    &TROPICAL_FISH,
    &TURTLE,
    &VEX,
    &VILLAGER,
    &VINDICATOR,
    &WANDERING_TRADER,
    &WARDEN,
    &WIND_CHARGE,
    &WITCH,
    &WITHER,
    &WITHER_SKELETON,
    &WITHER_SKULL,
    &WOLF,
    &ZOGLIN,
    &ZOMBIE,
    &ZOMBIE_HORSE,
    &ZOMBIE_NAUTILUS,
    &ZOMBIE_VILLAGER,
    &ZOMBIFIED_PIGLIN,
    &PLAYER,
    &FISHING_BOBBER,
];

pub fn register_all_entity_types(registry: &mut StaticRegistry<EntityType>) {
    for entity_type in ALL {
        registry.register(entity_type.identifier, entity_type);
    }
}

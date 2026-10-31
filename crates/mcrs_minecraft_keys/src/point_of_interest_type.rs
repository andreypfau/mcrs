// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_resource::{ResourceLocation, rl};

pub const ARMORER: ResourceLocation<'static> = rl!("minecraft:armorer");
pub const BUTCHER: ResourceLocation<'static> = rl!("minecraft:butcher");
pub const CARTOGRAPHER: ResourceLocation<'static> = rl!("minecraft:cartographer");
pub const CLERIC: ResourceLocation<'static> = rl!("minecraft:cleric");
pub const FARMER: ResourceLocation<'static> = rl!("minecraft:farmer");
pub const FISHERMAN: ResourceLocation<'static> = rl!("minecraft:fisherman");
pub const FLETCHER: ResourceLocation<'static> = rl!("minecraft:fletcher");
pub const LEATHERWORKER: ResourceLocation<'static> = rl!("minecraft:leatherworker");
pub const LIBRARIAN: ResourceLocation<'static> = rl!("minecraft:librarian");
pub const MASON: ResourceLocation<'static> = rl!("minecraft:mason");
pub const SHEPHERD: ResourceLocation<'static> = rl!("minecraft:shepherd");
pub const TOOLSMITH: ResourceLocation<'static> = rl!("minecraft:toolsmith");
pub const WEAPONSMITH: ResourceLocation<'static> = rl!("minecraft:weaponsmith");
pub const HOME: ResourceLocation<'static> = rl!("minecraft:home");
pub const MEETING: ResourceLocation<'static> = rl!("minecraft:meeting");
pub const BEEHIVE: ResourceLocation<'static> = rl!("minecraft:beehive");
pub const BEE_NEST: ResourceLocation<'static> = rl!("minecraft:bee_nest");
pub const NETHER_PORTAL: ResourceLocation<'static> = rl!("minecraft:nether_portal");
pub const LODESTONE: ResourceLocation<'static> = rl!("minecraft:lodestone");
pub const LIGHTNING_ROD: ResourceLocation<'static> = rl!("minecraft:lightning_rod");
pub const TEST_INSTANCE: ResourceLocation<'static> = rl!("minecraft:test_instance");

pub const ENTRIES: &[ResourceLocation<'static>] = &[
    ARMORER,
    BUTCHER,
    CARTOGRAPHER,
    CLERIC,
    FARMER,
    FISHERMAN,
    FLETCHER,
    LEATHERWORKER,
    LIBRARIAN,
    MASON,
    SHEPHERD,
    TOOLSMITH,
    WEAPONSMITH,
    HOME,
    MEETING,
    BEEHIVE,
    BEE_NEST,
    NETHER_PORTAL,
    LODESTONE,
    LIGHTNING_ROD,
    TEST_INSTANCE,
];

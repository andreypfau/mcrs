// Written by `cargo run -p mcrs_minecraft_update -- keys`; do not edit.

use mcrs_minecraft_core::{StaticResourceLocation, rl};

pub const ARMORER: StaticResourceLocation = rl!("minecraft:armorer");
pub const BUTCHER: StaticResourceLocation = rl!("minecraft:butcher");
pub const CARTOGRAPHER: StaticResourceLocation = rl!("minecraft:cartographer");
pub const CLERIC: StaticResourceLocation = rl!("minecraft:cleric");
pub const FARMER: StaticResourceLocation = rl!("minecraft:farmer");
pub const FISHERMAN: StaticResourceLocation = rl!("minecraft:fisherman");
pub const FLETCHER: StaticResourceLocation = rl!("minecraft:fletcher");
pub const LEATHERWORKER: StaticResourceLocation = rl!("minecraft:leatherworker");
pub const LIBRARIAN: StaticResourceLocation = rl!("minecraft:librarian");
pub const MASON: StaticResourceLocation = rl!("minecraft:mason");
pub const SHEPHERD: StaticResourceLocation = rl!("minecraft:shepherd");
pub const TOOLSMITH: StaticResourceLocation = rl!("minecraft:toolsmith");
pub const WEAPONSMITH: StaticResourceLocation = rl!("minecraft:weaponsmith");
pub const HOME: StaticResourceLocation = rl!("minecraft:home");
pub const MEETING: StaticResourceLocation = rl!("minecraft:meeting");
pub const BEEHIVE: StaticResourceLocation = rl!("minecraft:beehive");
pub const BEE_NEST: StaticResourceLocation = rl!("minecraft:bee_nest");
pub const NETHER_PORTAL: StaticResourceLocation = rl!("minecraft:nether_portal");
pub const LODESTONE: StaticResourceLocation = rl!("minecraft:lodestone");
pub const TEST_INSTANCE: StaticResourceLocation = rl!("minecraft:test_instance");
pub const LIGHTNING_ROD: StaticResourceLocation = rl!("minecraft:lightning_rod");

pub const ENTRIES: &[StaticResourceLocation] = &[
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
    TEST_INSTANCE,
    LIGHTNING_ROD,
];

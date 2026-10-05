// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const ADMIRE_ITEM: Id<crate::Activity> = Id::from_static(12);
pub const AVOID: Id<crate::Activity> = Id::from_static(13);
pub const CELEBRATE: Id<crate::Activity> = Id::from_static(11);
pub const CORE: Id<crate::Activity> = Id::from_static(0);
pub const DIG: Id<crate::Activity> = Id::from_static(25);
pub const EMERGE: Id<crate::Activity> = Id::from_static(24);
pub const FIGHT: Id<crate::Activity> = Id::from_static(10);
pub const HIDE: Id<crate::Activity> = Id::from_static(9);
pub const IDLE: Id<crate::Activity> = Id::from_static(1);
pub const INVESTIGATE: Id<crate::Activity> = Id::from_static(22);
pub const LAY_SPAWN: Id<crate::Activity> = Id::from_static(20);
pub const LONG_JUMP: Id<crate::Activity> = Id::from_static(16);
pub const MEET: Id<crate::Activity> = Id::from_static(5);
pub const PANIC: Id<crate::Activity> = Id::from_static(6);
pub const PLAY: Id<crate::Activity> = Id::from_static(3);
pub const PLAY_DEAD: Id<crate::Activity> = Id::from_static(15);
pub const PRE_RAID: Id<crate::Activity> = Id::from_static(8);
pub const RAID: Id<crate::Activity> = Id::from_static(7);
pub const RAM: Id<crate::Activity> = Id::from_static(17);
pub const REST: Id<crate::Activity> = Id::from_static(4);
pub const RIDE: Id<crate::Activity> = Id::from_static(14);
pub const ROAR: Id<crate::Activity> = Id::from_static(23);
pub const SNIFF: Id<crate::Activity> = Id::from_static(21);
pub const SWIM: Id<crate::Activity> = Id::from_static(19);
pub const TONGUE: Id<crate::Activity> = Id::from_static(18);
pub const WORK: Id<crate::Activity> = Id::from_static(2);

pub const NAMES: &[&str] = &[
    "minecraft:core",
    "minecraft:idle",
    "minecraft:work",
    "minecraft:play",
    "minecraft:rest",
    "minecraft:meet",
    "minecraft:panic",
    "minecraft:raid",
    "minecraft:pre_raid",
    "minecraft:hide",
    "minecraft:fight",
    "minecraft:celebrate",
    "minecraft:admire_item",
    "minecraft:avoid",
    "minecraft:ride",
    "minecraft:play_dead",
    "minecraft:long_jump",
    "minecraft:ram",
    "minecraft:tongue",
    "minecraft:swim",
    "minecraft:lay_spawn",
    "minecraft:sniff",
    "minecraft:investigate",
    "minecraft:roar",
    "minecraft:emerge",
    "minecraft:dig",
];

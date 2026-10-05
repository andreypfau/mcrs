// Written by `cargo run -p mcrs_minecraft_update -- names`; do not edit.

use mcrs_minecraft_registry::Id;

pub const ALTER_GROUND: Id<crate::TreeDecoratorType> = Id::from_static(7);
pub const ATTACHED_TO_LEAVES: Id<crate::TreeDecoratorType> = Id::from_static(8);
pub const ATTACHED_TO_LOGS: Id<crate::TreeDecoratorType> = Id::from_static(10);
pub const BEEHIVE: Id<crate::TreeDecoratorType> = Id::from_static(6);
pub const COCOA: Id<crate::TreeDecoratorType> = Id::from_static(4);
pub const CREAKING_HEART: Id<crate::TreeDecoratorType> = Id::from_static(3);
pub const LEAVE_VINE: Id<crate::TreeDecoratorType> = Id::from_static(1);
pub const PALE_MOSS: Id<crate::TreeDecoratorType> = Id::from_static(2);
pub const PLACE_ON_GROUND: Id<crate::TreeDecoratorType> = Id::from_static(9);
pub const SHELF_MUSHROOM: Id<crate::TreeDecoratorType> = Id::from_static(5);
pub const TRUNK_VINE: Id<crate::TreeDecoratorType> = Id::from_static(0);

pub const NAMES: &[&str] = &[
    "minecraft:trunk_vine",
    "minecraft:leave_vine",
    "minecraft:pale_moss",
    "minecraft:creaking_heart",
    "minecraft:cocoa",
    "minecraft:shelf_mushroom",
    "minecraft:beehive",
    "minecraft:alter_ground",
    "minecraft:attached_to_leaves",
    "minecraft:place_on_ground",
    "minecraft:attached_to_logs",
];

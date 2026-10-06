use crate::keys::Owner;

pub const OWNERS: &[Owner] = &[
    Owner {
        registry: "minecraft:timeline",
        krate: "mcrs_minecraft_environment",
        value: "crate::timeline::Timeline",
    },
    Owner {
        registry: "minecraft:world_clock",
        krate: "mcrs_minecraft_environment",
        value: "crate::world_clock::WorldClock",
    },
];

pub const ARROW: u32 = 0;
pub const CACTUS: u32 = 2;
pub const CAMPFIRE: u32 = 3;
pub const CRAMMING: u32 = 4;
pub const DRAGON_BREATH: u32 = 5;
pub const DROWN: u32 = 6;
pub const EXPLOSION: u32 = 9;
pub const FALL: u32 = 10;
pub const FIREBALL: u32 = 14;
pub const FLY_INTO_WALL: u32 = 16;
pub const FREEZE: u32 = 17;
pub const GENERIC: u32 = 18;
pub const GENERIC_KILL: u32 = 19;
pub const HOT_FLOOR: u32 = 20;
pub const IN_FIRE: u32 = 21;
pub const IN_WALL: u32 = 22;
pub const INDIRECT_MAGIC: u32 = 23;
pub const LAVA: u32 = 24;
pub const MAGIC: u32 = 27;
pub const MOB_ATTACK: u32 = 28;
pub const MOB_ATTACK_NO_AGGRO: u32 = 29;
pub const MOB_PROJECTILE: u32 = 30;
pub const ON_FIRE: u32 = 31;
pub const OUT_OF_WORLD: u32 = 32;
pub const OUTSIDE_BORDER: u32 = 33;
pub const PLAYER_ATTACK: u32 = 34;
pub const PLAYER_EXPLOSION: u32 = 35;
pub const SPIT: u32 = 38;
pub const STALAGMITE: u32 = 39;
pub const STARVE: u32 = 40;
pub const STING: u32 = 41;
pub const SWEET_BERRY_BUSH: u32 = 43;
pub const THORNS: u32 = 44;
pub const THROWN: u32 = 45;
pub const TRIDENT: u32 = 46;
pub const UNATTRIBUTED_FIREBALL: u32 = 47;
pub const WIND_CHARGE: u32 = 48;
pub const WITHER: u32 = 49;
pub const WITHER_SKULL: u32 = 50;

#[must_use]
pub const fn message_id(damage_type: u32) -> &'static str {
    match damage_type {
        ARROW => "arrow",
        1 => "badRespawnPoint",
        CACTUS => "cactus",
        CAMPFIRE | IN_FIRE => "inFire",
        CRAMMING => "cramming",
        DRAGON_BREATH => "dragonBreath",
        DROWN => "drown",
        7 => "dryOut",
        8 => "enderPearl",
        EXPLOSION | PLAYER_EXPLOSION => "explosion",
        FALL => "fall",
        11 => "anvil",
        12 => "fallingBlock",
        13 => "fallingStalactite",
        FIREBALL => "fireball",
        15 => "fireworks",
        FLY_INTO_WALL => "flyIntoWall",
        FREEZE => "freeze",
        GENERIC => "generic",
        GENERIC_KILL => "genericKill",
        HOT_FLOOR => "hotFloor",
        IN_WALL => "inWall",
        INDIRECT_MAGIC => "indirectMagic",
        LAVA => "lava",
        25 => "lightningBolt",
        26 => "mace_smash",
        MAGIC => "magic",
        MOB_ATTACK | MOB_ATTACK_NO_AGGRO | MOB_PROJECTILE | SPIT | WIND_CHARGE => "mob",
        ON_FIRE | UNATTRIBUTED_FIREBALL => "onFire",
        OUT_OF_WORLD => "outOfWorld",
        OUTSIDE_BORDER => "outsideBorder",
        PLAYER_ATTACK => "player",
        36 => "sonic_boom",
        37 => "spear",
        STALAGMITE => "stalagmite",
        STARVE => "starve",
        STING => "sting",
        42 => "sulfurCubeHot",
        SWEET_BERRY_BUSH => "sweetBerryBush",
        THORNS => "thorns",
        THROWN => "thrown",
        TRIDENT => "trident",
        WITHER => "wither",
        WITHER_SKULL => "witherSkull",
        _ => "generic",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// DMG-SRC-001: the death message of a damage type names the key of the damage type, not the
    /// key of a neighbouring type.
    #[test]
    fn the_death_keys_follow_the_registry_order() {
        assert_eq!(message_id(7), "dryOut");
        assert_eq!(message_id(8), "enderPearl");
        assert_eq!(message_id(EXPLOSION), "explosion");
        assert_eq!(message_id(FALL), "fall");
        assert_eq!(message_id(IN_FIRE), "inFire");
        assert_eq!(message_id(ON_FIRE), "onFire");
        assert_eq!(message_id(SWEET_BERRY_BUSH), "sweetBerryBush");
        assert_eq!(message_id(WITHER_SKULL), "witherSkull");
    }
}

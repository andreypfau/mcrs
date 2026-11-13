mod dig;
mod rate;

use mcrs_minecraft_block_break::Miner;

const STONE: f32 = 1.5;

const EMPTY_HAND: Miner = Miner {
    item_speed: 1.0,
    harvests: false,
    on_ground: true,
    eyes_in_water: false,
};

const WOODEN_PICKAXE: Miner = Miner {
    item_speed: 2.0,
    harvests: true,
    on_ground: true,
    eyes_in_water: false,
};

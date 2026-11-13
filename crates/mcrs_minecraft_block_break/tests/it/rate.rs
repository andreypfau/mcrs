use mcrs_minecraft_block_break::Miner;
use mcrs_minecraft_block_break::rate;

use crate::EMPTY_HAND;
use crate::STONE;
use crate::WOODEN_PICKAXE;

#[test]
fn brk_prg_002_water_and_a_clear_ground_flag_slow_the_rate() {
    struct Case {
        name: &'static str,
        miner: Miner,
        rate: f32,
        half_last_digit: f32,
    }
    let cases = [
        Case {
            name: "no factor",
            miner: WOODEN_PICKAXE,
            rate: 0.0444,
            half_last_digit: 0.000_05,
        },
        Case {
            name: "on-ground flag clear",
            miner: Miner {
                on_ground: false,
                ..WOODEN_PICKAXE
            },
            rate: 0.008_89,
            half_last_digit: 0.000_005,
        },
        Case {
            name: "eyes in water",
            miner: Miner {
                eyes_in_water: true,
                ..WOODEN_PICKAXE
            },
            rate: 0.008_89,
            half_last_digit: 0.000_005,
        },
    ];
    for case in cases {
        let got = rate(STONE, &case.miner);
        assert!(
            (got - case.rate).abs() < case.half_last_digit,
            "{}: the rate {got} does not round to the captured {}",
            case.name,
            case.rate
        );
    }
}

#[test]
fn brk_srv_002_a_hardness_of_zero_breaks_on_the_start_with_each_item() {
    let miners = [
        EMPTY_HAND,
        Miner {
            on_ground: false,
            eyes_in_water: true,
            ..EMPTY_HAND
        },
        Miner {
            item_speed: 0.0,
            ..EMPTY_HAND
        },
    ];
    for miner in miners {
        let got = rate(0.0, &miner);
        assert!(got >= 1.0, "{miner:?}: the rate {got} starts a dig");
    }
}

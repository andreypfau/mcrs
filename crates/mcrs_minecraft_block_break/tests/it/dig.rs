use mcrs_minecraft_block_break::Dig;
use mcrs_minecraft_block_break::Finish;
use mcrs_minecraft_block_break::Miner;
use mcrs_minecraft_block_break::rate;

use crate::EMPTY_HAND;
use crate::STONE;
use crate::WOODEN_PICKAXE;

fn dig_after(ticks: u32, rate: f32) -> Dig {
    let mut dig = Dig::start(rate);
    for _ in 0..ticks {
        dig.advance(rate);
    }
    dig
}

/// The tick count where the stage first reaches `stage`, or 1000 when it does not.
fn first_tick_with_stage(rate: f32, stage: u8) -> u32 {
    let mut dig = Dig::start(rate);
    while dig.stage() < stage && dig.ticks() < 1000 {
        dig.advance(rate);
    }
    dig.ticks()
}

struct FirstStage {
    block: &'static str,
    hardness: f32,
    item_speed: f32,
    harvests: bool,
    stage: u8,
    tick: u32,
}

#[rustfmt::skip]
const CAPTURED_FIRST_STAGES: [FirstStage; 17] = [
    FirstStage { block: "stone",      hardness: 1.5,  item_speed: 1.0,  harvests: false, stage: 1,  tick: 14 },
    FirstStage { block: "stone",      hardness: 1.5,  item_speed: 1.0,  harvests: false, stage: 9,  tick: 134 },
    FirstStage { block: "stone",      hardness: 1.5,  item_speed: 1.0,  harvests: false, stage: 10, tick: 149 },
    FirstStage { block: "stone",      hardness: 1.5,  item_speed: 1.0,  harvests: false, stage: 13, tick: 194 },
    FirstStage { block: "stone",      hardness: 1.5,  item_speed: 2.0,  harvests: true,  stage: 10, tick: 22 },
    FirstStage { block: "stone",      hardness: 1.5,  item_speed: 8.0,  harvests: true,  stage: 10, tick: 5 },
    FirstStage { block: "dirt",       hardness: 0.5,  item_speed: 1.0,  harvests: true,  stage: 10, tick: 14 },
    FirstStage { block: "dirt",       hardness: 0.5,  item_speed: 2.0,  harvests: true,  stage: 10, tick: 7 },
    FirstStage { block: "obsidian",   hardness: 50.0, item_speed: 8.0,  harvests: true,  stage: 10, tick: 187 },
    FirstStage { block: "iron ore",   hardness: 3.0,  item_speed: 2.0,  harvests: false, stage: 10, tick: 149 },
    FirstStage { block: "iron ore",   hardness: 3.0,  item_speed: 4.0,  harvests: true,  stage: 10, tick: 22 },
    FirstStage { block: "glass",      hardness: 0.3,  item_speed: 1.0,  harvests: true,  stage: 10, tick: 8 },
    FirstStage { block: "cobweb",     hardness: 4.0,  item_speed: 15.0, harvests: true,  stage: 10, tick: 7 },
    FirstStage { block: "cobweb",     hardness: 4.0,  item_speed: 1.0,  harvests: false, stage: 1,  tick: 40 },
    FirstStage { block: "cobweb",     hardness: 4.0,  item_speed: 1.0,  harvests: false, stage: 7,  tick: 279 },
    FirstStage { block: "white wool", hardness: 0.8,  item_speed: 5.0,  harvests: true,  stage: 10, tick: 4 },
    FirstStage { block: "oak leaves", hardness: 0.2,  item_speed: 2.0,  harvests: true,  stage: 10, tick: 2 },
];

#[test]
fn brk_prg_001_a_stage_first_shows_within_a_tick_of_the_captured_tick_count() {
    for case in CAPTURED_FIRST_STAGES {
        let miner = Miner {
            item_speed: case.item_speed,
            harvests: case.harvests,
            ..EMPTY_HAND
        };
        let got = first_tick_with_stage(rate(case.hardness, &miner), case.stage);
        assert!(
            got == case.tick || got == case.tick + 1,
            "{} with the speed {}: the stage {} first shows at {got}, the captured product at {}",
            case.block,
            case.item_speed,
            case.stage,
            case.tick
        );
    }
}

#[test]
fn brk_prg_001_the_progress_is_a_sum_and_not_a_product() {
    let got = first_tick_with_stage(rate(STONE, &EMPTY_HAND), 2);
    assert_eq!(got, 30, "the product of the reference gives 29");
}

#[test]
fn brk_prg_001_each_tick_adds_its_own_rate() {
    let mut dig = Dig::start(0.25);
    dig.advance(0.5);
    assert_eq!(dig.ticks(), 1, "one tick follows the start action");
    assert_eq!(dig.progress(), 0.75, "both rates are exact in binary");
}

#[test]
fn brk_srv_005_the_stage_wraps_at_256() {
    struct Seen {
        tick: u32,
        stage: u8,
    }
    let seen = [
        Seen { tick: 0, stage: 7 },
        Seen {
            tick: 32,
            stage: 249,
        },
        Seen { tick: 33, stage: 0 },
        Seen { tick: 34, stage: 8 },
        Seen {
            tick: 66,
            stage: 250,
        },
        Seen { tick: 67, stage: 1 },
        Seen { tick: 68, stage: 9 },
    ];
    // A diamond pickaxe with Efficiency V has the speed 8 + 26.
    let miner = Miner {
        item_speed: 34.0,
        ..WOODEN_PICKAXE
    };
    let rate = rate(STONE, &miner);
    for case in seen {
        let got = dig_after(case.tick, rate).stage();
        assert_eq!(got, case.stage, "the stage at the tick count {}", case.tick);
    }
}

#[test]
fn brk_srv_008_a_finish_breaks_only_from_seven_tenths_of_progress() {
    let rate = rate(STONE, &WOODEN_PICKAXE);
    for tick in [5, 13, 14] {
        let got = dig_after(tick, rate).finish(rate);
        assert_eq!(got, Finish::DelayedBreak, "after the tick count {tick}");
    }
    for tick in [15, 16] {
        let got = dig_after(tick, rate).finish(rate);
        assert_eq!(got, Finish::Break, "after the tick count {tick}");
    }
}

#[test]
fn brk_srv_011_a_hardness_of_minus_one_never_breaks() {
    let rate = rate(-1.0, &EMPTY_HAND);
    assert_eq!(rate, 0.0, "the rate of a hardness of -1");
    let dig = dig_after(30, rate);
    assert_eq!(dig.stage(), 0, "the stage after 30 ticks");
    assert_eq!(
        dig.finish(rate),
        Finish::Stay,
        "a finish makes no delayed break"
    );
}

//! The goals of the pig in a pen, against the timings and the chances of `mob-goals.md`.

use mcrs_minecraft_brain::AiDelay;
use mcrs_minecraft_brain::ConfigError;
use mcrs_minecraft_brain::DamageCause;
use mcrs_minecraft_brain::OrderKey;
use mcrs_minecraft_brain::Panic;
use mcrs_minecraft_brain::PlayerFlags;
use mcrs_minecraft_brain::PlayerId;
use mcrs_minecraft_brain::PlayerRow;
use mcrs_minecraft_brain::Point;
use mcrs_minecraft_brain::StageConfig;
use mcrs_minecraft_brain::WorldSeed;
use mcrs_minecraft_goals::IdleRule;
use mcrs_minecraft_goals::PenConfig;
use mcrs_minecraft_goals::PenLook;
use mcrs_minecraft_goals::PigPen;
use mcrs_minecraft_goals::run_tiles;

/// The steady speed on grass of a pig with the speed modifier `m` (GOAL-CTL-002): E-G2 measures
/// 0.2150 for the panic and 0.1377 for the stroll.
fn walk_speed(modifier: f64) -> f64 {
    let speed = modifier * 0.25;
    speed * speed * 2.2026
}

fn is_speed(walk: Option<f64>, modifier: f64) -> bool {
    walk.is_some_and(|speed| (speed - walk_speed(modifier)).abs() < 1e-9)
}

fn pen(pigs: u32, side: i32, idle: IdleRule) -> Result<PigPen, ConfigError> {
    let config = PenConfig {
        side,
        pigs,
        players: 0,
        hurt_chance: 0.0,
        idle,
        seed: 41,
    };
    let stage = StageConfig::new(WorldSeed(41)).with_delay(2, AiDelay::Fixed(1))?;
    Ok(PigPen::new(config, stage))
}

#[test]
fn a_hurt_pig_panics_at_its_next_evaluation_and_starts_paths_only_while_the_record_is_fresh() {
    let pigs = 1_000;
    let mut pen = pen(pigs, 512, IdleRule::AlwaysFar).unwrap();
    while pen.tick() < 20 {
        pen.step(run_tiles);
    }
    for index in 0..pigs as usize {
        pen.hurt(index, DamageCause::Panic);
    }
    let hit = 19;
    let mut starts = vec![Vec::new(); pigs as usize];
    let mut walking = vec![false; pigs as usize];
    while pen.tick() <= 130 {
        let tick = pen.step(run_tiles).tick;
        for (index, was) in walking.iter_mut().enumerate() {
            let now = is_speed(pen.pig(index).unwrap().walk_speed, 1.25);
            if now && !*was {
                starts[index].push(tick);
            }
            *was = now;
        }
    }
    let prompt = starts
        .iter()
        .filter(|ticks| ticks.first().is_some_and(|&tick| tick <= hit + 3))
        .count() as f64
        / f64::from(pigs);
    assert!(
        (0.64..=0.74).contains(&prompt),
        "the first evaluation after the hit starts the panic unless all 10 tries of D(5, 4) fail, \
         1 - (8/9)^10 = 0.69; got {prompt}"
    );
    let last = starts.iter().flatten().max().copied().unwrap();
    assert!(
        last <= hit + 40 + 1,
        "a panic path starts only while the record is fresh, issued at most 40 ticks after the \
         hit and applied one tick later; the last start is at {last}"
    );
    let restarted = starts.iter().filter(|ticks| ticks.len() >= 2).count();
    assert!(
        restarted * 2 > pigs as usize,
        "a path that ends while the record is fresh starts the panic again: {restarted} pigs"
    );
    for index in 0..pigs as usize {
        let pig = pen.pig(index).unwrap();
        assert_eq!(
            pig.panic,
            Panic::Calm,
            "pig {index} still panics at tick 130"
        );
        assert!(!is_speed(pig.walk_speed, 1.25));
    }
}

#[test]
fn the_stroll_starts_after_a_geometric_count_of_evaluations_with_the_chance_one_in_sixty() {
    let pigs = 800;
    let mut pen = pen(pigs, 1024, IdleRule::AlwaysNear).unwrap();
    let mut ended = vec![None; pigs as usize];
    let mut walking = vec![false; pigs as usize];
    let mut gaps = Vec::new();
    while pen.tick() <= 3_000 {
        let tick = pen.step(run_tiles).tick;
        for (index, was) in walking.iter_mut().enumerate() {
            let walk = pen.pig(index).unwrap().walk_speed;
            let now = walk.is_some();
            if now && !*was {
                assert!(is_speed(walk, 1.0), "only the stroll walks without a hit");
                if let Some(end) = ended[index] {
                    gaps.push(tick - end);
                }
            }
            if !now && *was {
                ended[index] = Some(tick);
            }
            *was = now;
        }
    }
    let count = gaps.len() as f64;
    let mean = gaps.iter().sum::<u64>() as f64 / count;
    assert!(
        count > 8_000.0 && (114.0..=127.0).contains(&mean),
        "the gap from the end of a stroll to the next start is 1 or 2 ticks to the next \
         evaluation, 2 ticks for each of the 59 failed gates on average and 1 tick of delay: \
         120.5; got {mean} over {count} gaps"
    );
    let first = gaps.iter().filter(|&&gap| gap <= 3).count() as f64 / count;
    assert!(
        (0.010..=0.024).contains(&first),
        "the gate passes in the first examined evaluation with the chance 1/60; got {first}"
    );
}

#[test]
fn a_pig_looks_at_a_player_within_six_blocks_and_never_at_one_farther() {
    let pigs = 24;
    let mut pen = pen(pigs, 128, IdleRule::AlwaysFar).unwrap();
    let centre = Point::new(64.0, 0.0, 64.0);
    for index in 0..pigs as usize {
        let angle = index as f64 * 0.7;
        let distance = if index % 2 == 0 { 5.9 } else { 6.1 };
        let position = Point::new(
            centre.x + distance * angle.cos(),
            0.0,
            centre.z + distance * angle.sin(),
        );
        pen.place_pig(index, position);
    }
    pen.set_players(vec![PlayerRow {
        player: PlayerId(1),
        key: OrderKey::new(1 << 64 | 1),
        position: centre,
        eye_height: 1.62,
        flags: PlayerFlags::ALIVE,
        visibility: 1.0,
        vehicle: None,
    }]);
    let mut looked = vec![false; pigs as usize];
    while pen.tick() <= 2_000 {
        pen.step(run_tiles);
        for (index, seen) in looked.iter_mut().enumerate() {
            *seen |= pen.pig(index).unwrap().look == PenLook::AtPlayer;
        }
    }
    for (index, seen) in looked.iter().enumerate() {
        assert_eq!(*seen, index % 2 == 0, "pig {index} at 5.9 or 6.1 blocks");
    }
}

//! The goals of the pig in a pen, against the timings and the chances of `mob-goals.md`.

use glam::DVec3;
use mcrs_minecraft_brain::AiDelay;
use mcrs_minecraft_brain::ConfigError;
use mcrs_minecraft_brain::DamageCause;
use mcrs_minecraft_brain::Panic;
use mcrs_minecraft_brain::PlayerFlags;
use mcrs_minecraft_brain::PlayerRow;
use mcrs_minecraft_brain::StageConfig;
use mcrs_minecraft_brain::WorldSeed;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_key::EntityKey;
use mcrs_minecraft_goals::IdleRule;
use mcrs_minecraft_goals::PenConfig;
use mcrs_minecraft_goals::PenLook;
use mcrs_minecraft_goals::PigPen;
use mcrs_minecraft_goals::Pond;
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
    let centre = DVec3::new(64.0, 0.0, 64.0);
    for index in 0..pigs as usize {
        let angle = index as f64 * 0.7;
        let distance = if index % 2 == 0 { 5.9 } else { 6.1 };
        let position = DVec3::new(
            centre.x + distance * angle.cos(),
            0.0,
            centre.z + distance * angle.sin(),
        );
        pen.place_pig(index, position);
    }
    pen.set_players(vec![PlayerRow {
        player: EntityId(1),
        key: EntityKey(1 << 64 | 1),
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

/// GOAL-STR-002: a pig in water searches land with `L(15, 7)`. From the middle of a pond whose
/// shore is 12 or more blocks away, the search on land `L(10, 7)` finds nothing, and `D(10, 7)`
/// rejects water by its malus, so only the search in water starts a stroll.
#[test]
fn a_pig_in_the_middle_of_a_pond_strolls_to_land_beyond_ten_blocks() {
    let pond = Pond {
        min_x: 20,
        min_z: 20,
        max_x: 43,
        max_z: 43,
    };
    let mut pen = pen(1, 64, IdleRule::AlwaysNear).unwrap();
    pen.flood(pond);
    let start = DVec3::new(32.5, 0.0, 32.5);
    pen.place_pig(0, start);
    let mut target = None;
    while target.is_none() && pen.tick() <= 3_000 {
        pen.step(run_tiles);
        let pig = pen.pig(0).unwrap();
        if is_speed(pig.walk_speed, 1.0) {
            target = pig.walk_target;
        }
    }
    let target = target.expect("the pig strolls out of the pond");
    let block = (target.x.floor() as i32, target.z.floor() as i32);
    let in_pond = (pond.min_x..=pond.max_x).contains(&block.0)
        && (pond.min_z..=pond.max_z).contains(&block.1);
    assert!(!in_pond, "the stroll ends on land: {target}");
    let reach = (target.x - start.x).abs().max((target.z - start.z).abs());
    assert!(
        (10.5..=15.5).contains(&reach),
        "the land lies beyond the 10 blocks of L(10, 7) and within the 15 of L(15, 7): {target}"
    );
}

/// GOAL-PNC-002, GOAL-PNC-003, E-G12: a burning pig with a fresh panic record runs to the nearest
/// water block, the block position itself, not to a position of `D(5, 4)`.
#[test]
fn a_burning_pig_panics_into_the_nearest_water() {
    let mut pen = pen(1, 64, IdleRule::AlwaysFar).unwrap();
    pen.flood(Pond {
        min_x: 13,
        min_z: 20,
        max_x: 36,
        max_z: 43,
    });
    pen.place_pig(0, DVec3::new(10.5, 0.0, 32.5));
    while pen.tick() < 20 {
        pen.step(run_tiles);
    }
    pen.set_on_fire(0, true);
    pen.hurt(0, DamageCause::Panic);
    let mut target = None;
    while target.is_none() && pen.tick() <= 30 {
        pen.step(run_tiles);
        let pig = pen.pig(0).unwrap();
        if is_speed(pig.walk_speed, 1.25) {
            target = pig.walk_target;
        }
    }
    assert_eq!(
        target,
        Some(DVec3::new(13.5, 0.0, 32.5)),
        "the panic walks into the water 3 blocks east"
    );
}

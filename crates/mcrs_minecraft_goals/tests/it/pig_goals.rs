use glam::DVec3;
use mcrs_minecraft_brain::AiDelay;
use mcrs_minecraft_brain::Cadence;
use mcrs_minecraft_brain::ConfigError;
use mcrs_minecraft_brain::ControlFlags;
use mcrs_minecraft_brain::DamageCause;
use mcrs_minecraft_brain::DamageRecord;
use mcrs_minecraft_brain::Intents;
use mcrs_minecraft_brain::MobBrain;
use mcrs_minecraft_brain::MobFlags;
use mcrs_minecraft_brain::MobView;
use mcrs_minecraft_brain::Panic;
use mcrs_minecraft_brain::PathIntent;
use mcrs_minecraft_brain::PlayerFlags;
use mcrs_minecraft_brain::PlayerRow;
use mcrs_minecraft_brain::StageConfig;
use mcrs_minecraft_brain::Turn;
use mcrs_minecraft_brain::WorldSeed;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_key::EntityUuid;
use mcrs_minecraft_goals::ChickenBrain;
use mcrs_minecraft_goals::CowBrain;
use mcrs_minecraft_goals::FlatGround;
use mcrs_minecraft_goals::IdleRule;
use mcrs_minecraft_goals::PenConfig;
use mcrs_minecraft_goals::PenLook;
use mcrs_minecraft_goals::PigBrain;
use mcrs_minecraft_goals::PigPen;
use mcrs_minecraft_goals::Pond;
use mcrs_minecraft_goals::SheepBrain;
use mcrs_minecraft_goals::run_tiles;

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
fn hurt_pig_panics_at_next_evaluation_and_paths_while_fresh() {
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

fn stroll_gaps(pigs: u32, ticks: u64) -> Vec<u64> {
    let mut pen = pen(pigs, 1024, IdleRule::AlwaysNear).unwrap();
    let mut ended = vec![None; pigs as usize];
    let mut walking = vec![false; pigs as usize];
    let mut gaps = Vec::new();
    while pen.tick() <= ticks {
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
    gaps
}

fn gap_mean(gaps: &[u64]) -> f64 {
    gaps.iter().sum::<u64>() as f64 / gaps.len() as f64
}

fn first_gap_fraction(gaps: &[u64]) -> f64 {
    gaps.iter().filter(|&&gap| gap <= 3).count() as f64 / gaps.len() as f64
}

#[test]
fn a_short_run_of_strolls_has_the_geometric_mean_gap_within_four_standard_errors() {
    let gaps = stroll_gaps(200, 1_000);
    let count = gaps.len() as f64;
    let mean = gap_mean(&gaps);
    let variance = gaps
        .iter()
        .map(|&gap| (gap as f64 - mean).powi(2))
        .sum::<f64>()
        / (count - 1.0);
    let mean_error = 4.0 * (variance / count).sqrt();
    assert!(
        count > 500.0 && (mean - 120.5).abs() <= mean_error,
        "the mean gap is 120.5 ticks within {mean_error}; got {mean} over {count} gaps"
    );
}

mod exhaustive {
    use super::*;

    #[test]
    fn stroll_starts_after_geometric_count_with_chance_one_in_sixty() {
        let gaps = stroll_gaps(800, 3_000);
        let count = gaps.len() as f64;
        let mean = gap_mean(&gaps);
        assert!(
            count > 8_000.0 && (114.0..=127.0).contains(&mean),
            "the gap from the end of a stroll to the next start is 1 or 2 ticks to the next \
             evaluation, 2 ticks for each of the 59 failed gates on average and 1 tick of delay: \
             120.5; got {mean} over {count} gaps"
        );
        let first = first_gap_fraction(&gaps);
        assert!(
            (0.010..=0.024).contains(&first),
            "the gate passes in the first examined evaluation with the chance 1/60; got {first}"
        );
    }
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
        key: EntityUuid::from_u128(1 << 64 | 1),
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

fn panic_modifier<B: MobBrain<FlatGround> + Default>() -> Option<f32> {
    let ground = FlatGround {
        side: 64,
        pond: None,
    };
    (0..32u128).find_map(|key| {
        let view = MobView {
            mob: EntityId(0),
            key: EntityUuid::from_u128(key << 64),
            position: DVec3::new(32.5, 0.0, 32.5),
            eye_height: 1.0,
            flags: MobFlags::ALIVE.union(MobFlags::ON_GROUND),
            idle_time: 0,
            damage: Some(DamageRecord {
                tick: 19,
                cause: DamageCause::Panic,
            }),
            disabled: ControlFlags::NONE,
            path: Default::default(),
        };
        let turn = Turn {
            tick: 20,
            seed: WorldSeed(41),
            max_lag: 0,
            cadence: Cadence::new(1, 0),
            view: &view,
            players: &[],
            world: &ground,
        };
        let mut intents = Intents::default();
        B::default().evaluate(&turn, &mut intents);
        match intents.path {
            Some(PathIntent::Walk { speed_modifier, .. }) => Some(speed_modifier),
            _ => None,
        }
    })
}

#[test]
fn each_farm_animal_panics_at_the_speed_of_its_goal_list() {
    let cases = [
        ("pig", panic_modifier::<PigBrain>(), 1.25),
        ("cow", panic_modifier::<CowBrain>(), 2.0),
        ("sheep", panic_modifier::<SheepBrain>(), 1.25),
        ("chicken", panic_modifier::<ChickenBrain>(), 1.4),
    ];
    for (animal, modifier, expected) in cases {
        assert_eq!(modifier, Some(expected), "{animal}");
    }
}

//! The stage gives the same pigs and brains in each tick for any thread count and task split
//! (AIT-INV-001, AIT-INV-002), and the sleeping stage gives the decisions of the polling one.

use mcrs_minecraft_brain::AiDelay;
use mcrs_minecraft_brain::DamageCause;
use mcrs_minecraft_brain::OrderKey;
use mcrs_minecraft_brain::PlayerFlags;
use mcrs_minecraft_brain::PlayerId;
use mcrs_minecraft_brain::PlayerRow;
use mcrs_minecraft_brain::Point;
use mcrs_minecraft_brain::StageConfig;
use mcrs_minecraft_brain::TickContext;
use mcrs_minecraft_brain::TileWork;
use mcrs_minecraft_brain::WakeMode;
use mcrs_minecraft_brain::WorldSeed;
use mcrs_minecraft_goals::FlatGround;
use mcrs_minecraft_goals::HashScope;
use mcrs_minecraft_goals::IdleRule;
use mcrs_minecraft_goals::PenConfig;
use mcrs_minecraft_goals::PigBrain;
use mcrs_minecraft_goals::PigPen;
use mcrs_minecraft_goals::run_tiles;
use rayon::ThreadPool;
use rayon::ThreadPoolBuilder;
use rayon::prelude::*;

const TICKS: u32 = 200;

fn scene() -> PenConfig {
    PenConfig {
        side: 256,
        pigs: 4_000,
        players: 12,
        hurt_chance: 1.0 / 400.0,
        idle: IdleRule::FromPlayers,
        seed: 0x5EED,
    }
}

fn stage_config(wake: WakeMode) -> StageConfig {
    let config = StageConfig::new(WorldSeed(0x5EED)).with_wake(wake);
    config.with_delay(2, AiDelay::Fixed(1)).unwrap_or(config)
}

fn run_on(pool: &ThreadPool, tiles_per_task: usize, scope: HashScope, wake: WakeMode) -> Vec<u64> {
    let mut pen = PigPen::new(scene(), stage_config(wake));
    (0..TICKS)
        .map(|_| {
            pen.step(
                |work: &mut [TileWork<'_, PigBrain>], cx: &TickContext<'_, FlatGround>| {
                    pool.install(|| {
                        work.par_chunks_mut(tiles_per_task)
                            .for_each(|tasks| run_tiles(tasks, cx));
                    });
                },
            );
            pen.state_hash(scope)
        })
        .collect()
}

#[test]
fn one_eight_and_sixteen_threads_give_equal_hashes_in_each_tick() {
    let serial = {
        let mut pen = PigPen::new(scene(), stage_config(WakeMode::Sleep));
        let hashes: Vec<u64> = (0..TICKS)
            .map(|_| {
                pen.step(run_tiles);
                pen.state_hash(HashScope::Everything)
            })
            .collect();
        assert!(pen.decisions() > 1_000, "the scene makes decisions");
        hashes
    };
    for (threads, tiles_per_task) in [(1, 64), (8, 1), (8, 3), (16, 2), (16, 7)] {
        let pool = ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        let hashes = run_on(
            &pool,
            tiles_per_task,
            HashScope::Everything,
            WakeMode::Sleep,
        );
        let first_difference = hashes.iter().zip(&serial).position(|(a, b)| a != b);
        assert_eq!(
            first_difference, None,
            "{threads} threads with {tiles_per_task} tiles per task"
        );
    }
}

/// C-G5: a group of pigs takes a hit, and a player walks through the pen, so idle times reset,
/// strolls end by the idle time and pigs look at the player.
fn walk_and_hit(pen: &mut PigPen, tick: u64) {
    let mut players: Vec<PlayerRow> = (0..2)
        .map(|index| PlayerRow {
            player: PlayerId(100 + index),
            key: OrderKey::new(u128::from(100 + index) << 64 | 1),
            position: Point::new(f64::from(index as u32) * 200.0 + 20.0, 0.0, 30.0),
            eye_height: 1.62,
            flags: PlayerFlags::ALIVE,
            visibility: 1.0,
            vehicle: None,
        })
        .collect();
    players.push(PlayerRow {
        player: PlayerId(7),
        key: OrderKey::new(7 << 64 | 1),
        position: Point::new(tick as f64 * 0.4, 0.0, 128.0),
        eye_height: 1.62,
        flags: PlayerFlags::ALIVE,
        visibility: 0.8,
        vehicle: None,
    });
    pen.set_players(players);
    if tick == 150 {
        for index in 0..300 {
            pen.hurt(index, DamageCause::Panic);
        }
    }
}

fn run_scene(wake: WakeMode) -> (Vec<u64>, u64) {
    let config = PenConfig {
        hurt_chance: 1.0 / 2000.0,
        players: 0,
        ..scene()
    };
    let mut pen = PigPen::new(config, stage_config(wake));
    let hashes = (0..WALK_TICKS)
        .map(|_| {
            let tick = pen.tick();
            walk_and_hit(&mut pen, tick);
            pen.step(run_tiles);
            pen.state_hash(HashScope::Decisions)
        })
        .collect();
    (hashes, pen.stage().evaluations())
}

const WALK_TICKS: u32 = 640;

#[test]
fn sleep_gives_the_decisions_of_polling_tick_for_tick_with_fewer_evaluations() {
    let (sleep, asleep) = run_scene(WakeMode::Sleep);
    let (poll, awake) = run_scene(WakeMode::Poll);
    let first_difference = sleep.iter().zip(&poll).position(|(a, b)| a != b);
    assert_eq!(
        first_difference, None,
        "the first tick where sleep and poll differ"
    );
    assert!(
        asleep * 5 < awake,
        "sleep runs a fraction of the polled evaluations: {asleep} against {awake}"
    );
}

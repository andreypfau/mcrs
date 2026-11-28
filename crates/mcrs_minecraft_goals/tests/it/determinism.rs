use glam::DVec3;
use mcrs_minecraft_brain::AiDelay;
use mcrs_minecraft_brain::DamageCause;
use mcrs_minecraft_brain::PlayerFlags;
use mcrs_minecraft_brain::PlayerRow;
use mcrs_minecraft_brain::StageConfig;
use mcrs_minecraft_brain::TickContext;
use mcrs_minecraft_brain::TileWork;
use mcrs_minecraft_brain::WakeMode;
use mcrs_minecraft_brain::WorldSeed;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_key::EntityUuid;
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

fn run_on(
    pool: &ThreadPool,
    ticks: u32,
    tiles_per_task: usize,
    scope: HashScope,
    wake: WakeMode,
) -> Vec<u64> {
    let mut pen = PigPen::new(scene(), stage_config(wake));
    (0..ticks)
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

fn assert_thread_layouts_agree(ticks: u32, layouts: &[(usize, usize)]) -> u64 {
    let (serial, decisions) = {
        let mut pen = PigPen::new(scene(), stage_config(WakeMode::Sleep));
        let hashes: Vec<u64> = (0..ticks)
            .map(|_| {
                pen.step(run_tiles);
                pen.state_hash(HashScope::Everything)
            })
            .collect();
        (hashes, pen.decisions())
    };
    for &(threads, tiles_per_task) in layouts {
        let pool = ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        let hashes = run_on(
            &pool,
            ticks,
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
    decisions
}

fn walk_and_hit(pen: &mut PigPen, tick: u64, hit_tick: u64) {
    let mut players: Vec<PlayerRow> = (0..2)
        .map(|index| PlayerRow {
            player: EntityId(100 + index),
            key: EntityUuid::from_u128(u128::from(100 + index) << 64 | 1),
            position: DVec3::new(f64::from(index as u32) * 200.0 + 20.0, 0.0, 30.0),
            eye_height: 1.62,
            flags: PlayerFlags::ALIVE,
            visibility: 1.0,
            vehicle: None,
        })
        .collect();
    players.push(PlayerRow {
        player: EntityId(7),
        key: EntityUuid::from_u128(7 << 64 | 1),
        position: DVec3::new(tick as f64 * 0.4, 0.0, 128.0),
        eye_height: 1.62,
        flags: PlayerFlags::ALIVE,
        visibility: 0.8,
        vehicle: None,
    });
    pen.set_players(players);
    if tick == hit_tick {
        for index in 0..300 {
            pen.hurt(index, DamageCause::Panic);
        }
    }
}

fn run_scene(wake: WakeMode, ticks: u32, hit_tick: u64) -> (Vec<u64>, u64) {
    let config = PenConfig {
        hurt_chance: 1.0 / 2000.0,
        players: 0,
        ..scene()
    };
    let mut pen = PigPen::new(config, stage_config(wake));
    let hashes = (0..ticks)
        .map(|_| {
            let tick = pen.tick();
            walk_and_hit(&mut pen, tick, hit_tick);
            pen.step(run_tiles);
            pen.state_hash(HashScope::Decisions)
        })
        .collect();
    (hashes, pen.driver().evaluations())
}

fn assert_sleep_matches_poll(ticks: u32, hit_tick: u64) {
    let (sleep, asleep) = run_scene(WakeMode::Sleep, ticks, hit_tick);
    let (poll, awake) = run_scene(WakeMode::Poll, ticks, hit_tick);
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

#[test]
fn one_and_eight_threads_give_equal_hashes_in_a_short_run() {
    let decisions = assert_thread_layouts_agree(40, &[(1, 64), (8, 1)]);
    assert!(decisions > 0, "the scene makes decisions");
}

#[test]
fn sleep_gives_the_decisions_of_polling_in_a_short_walk() {
    assert_sleep_matches_poll(40, 20);
}

mod exhaustive {
    use super::*;

    #[test]
    fn one_eight_and_sixteen_threads_give_equal_hashes_in_each_tick() {
        let decisions =
            assert_thread_layouts_agree(200, &[(1, 64), (8, 1), (8, 3), (16, 2), (16, 7)]);
        assert!(decisions > 1_000, "the scene makes decisions");
    }

    #[test]
    fn sleep_gives_the_decisions_of_polling_tick_for_tick_with_fewer_evaluations() {
        assert_sleep_matches_poll(640, 150);
    }
}

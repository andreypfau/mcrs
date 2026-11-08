//! One stage tick of 50,000 pigs on 32 × 32 tiles, at 1, 8 and 16 threads: only the parallel map
//! of the tile tasks is timed, not the snapshot that the pen publishes nor its hot step. Each pig
//! acts as if a player stood near it, so it strolls (the most awake case of a pig); 64 players
//! stand in the pen, and a pig takes a hit about once in 4000 ticks. Before the timed runs, the
//! bench prints the full evaluations and the decisions of a tick, in the sleeping and the polling
//! stage.

use std::time::Duration;
use std::time::Instant;

use criterion::Criterion;
use criterion::criterion_group;
use criterion::criterion_main;
use mcrs_minecraft_brain::StageConfig;
use mcrs_minecraft_brain::TickContext;
use mcrs_minecraft_brain::TileWork;
use mcrs_minecraft_brain::WakeMode;
use mcrs_minecraft_brain::WorldSeed;
use mcrs_minecraft_goals::FlatGround;
use mcrs_minecraft_goals::IdleRule;
use mcrs_minecraft_goals::PenConfig;
use mcrs_minecraft_goals::PigBrain;
use mcrs_minecraft_goals::PigPen;
use mcrs_minecraft_goals::run_tiles;
use rayon::ThreadPool;
use rayon::ThreadPoolBuilder;
use rayon::prelude::*;

const PIGS: u32 = 50_000;
const WARM_UP_TICKS: u32 = 300;
const REPORT_TICKS: u32 = 400;

fn pen(wake: WakeMode) -> PigPen {
    let config = PenConfig {
        side: 1024,
        pigs: PIGS,
        players: 64,
        hurt_chance: 1.0 / 4000.0,
        idle: IdleRule::AlwaysNear,
        seed: 0xBE7C4,
    };
    let stage = StageConfig::new(WorldSeed(0xBE7C4)).with_wake(wake);
    let mut pen = PigPen::new(config, stage);
    for _ in 0..WARM_UP_TICKS {
        pen.step(run_tiles);
    }
    pen
}

/// Steps the pen once and returns the time of the parallel map of its tile tasks.
fn timed_step(pen: &mut PigPen, pool: &ThreadPool, threads: usize) -> Duration {
    let mut elapsed = Duration::ZERO;
    pen.step(
        |work: &mut [TileWork<'_, PigBrain>], cx: &TickContext<'_, FlatGround>| {
            let per_task = work.len().div_ceil(threads * 4).max(1);
            let start = Instant::now();
            pool.install(|| {
                work.par_chunks_mut(per_task)
                    .for_each(|tasks| run_tiles(tasks, cx));
            });
            elapsed = start.elapsed();
        },
    );
    elapsed
}

fn report(wake: WakeMode, pool: &ThreadPool, threads: usize) {
    let mut pen = pen(wake);
    let (evaluations, decisions) = (pen.stage().evaluations(), pen.decisions());
    let mut walking = 0;
    let mut stage = Duration::ZERO;
    for _ in 0..REPORT_TICKS {
        stage += timed_step(&mut pen, pool, threads);
        walking += pen.walking();
    }
    let ticks = f64::from(REPORT_TICKS);
    let evaluations = (pen.stage().evaluations() - evaluations) as f64 / ticks;
    let decisions = (pen.decisions() - decisions) as f64 / ticks;
    let per_tick = stage.as_secs_f64() / ticks;
    eprintln!(
        "{wake:?}, {threads} threads: {evaluations:.0} evaluations ({:.2}% of the pigs) and \
         {decisions:.0} decisions in a tick, {:.0} walking; stage {:.3} ms a tick, {:.2} M \
         decisions/s, {:.2} M evaluations/s",
        evaluations / f64::from(PIGS) * 100.0,
        walking as f64 / ticks,
        per_tick * 1e3,
        decisions / per_tick / 1e6,
        evaluations / per_tick / 1e6,
    );
}

fn pig_stage(criterion: &mut Criterion) {
    for threads in [1, 8, 16] {
        let Ok(pool) = ThreadPoolBuilder::new().num_threads(threads).build() else {
            eprintln!("no pool of {threads} threads");
            continue;
        };
        report(WakeMode::Sleep, &pool, threads);
        report(WakeMode::Poll, &pool, threads);
        let mut pen = pen(WakeMode::Sleep);
        criterion.bench_function(
            &format!("stage tick of 50k pigs, {threads} threads"),
            |bencher| {
                bencher.iter_custom(|iterations| {
                    (0..iterations)
                        .map(|_| timed_step(&mut pen, &pool, threads))
                        .sum()
                });
            },
        );
    }
}

criterion_group!(benches, pig_stage);
criterion_main!(benches);

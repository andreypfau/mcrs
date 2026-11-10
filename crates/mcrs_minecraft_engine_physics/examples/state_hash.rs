//! Prints a hash of the rows and the grid of the benchmark scene after a number of ticks, for a
//! comparison across thread counts and architectures:
//! `cargo run --release -p mcrs_minecraft_engine_physics --example state_hash -- <threads> <ticks>`.
#![expect(clippy::unwrap_used, reason = "a developer tool")]

use std::hash::DefaultHasher;
use std::hash::Hasher;

use bevy_tasks::TaskPoolBuilder;
use mcrs_minecraft_engine_physics::EntityWrites;
use mcrs_minecraft_engine_physics::TestScene;
use mcrs_minecraft_physics::Rest;

fn main() {
    let mut arguments = std::env::args().skip(1);
    let threads: usize = arguments.next().unwrap().parse().unwrap();
    let ticks: u32 = arguments.next().unwrap().parse().unwrap();
    let pool = TaskPoolBuilder::new().num_threads(threads).build();
    let mut scene = TestScene::<()>::new(16, 50_000, 50_000, 11);
    let tiles_per_task = (32 * 32_usize).div_ceil(threads * 4);
    let mut writes = EntityWrites::default();
    for _ in 0..ticks {
        scene.step(&pool, tiles_per_task, &mut writes);
    }
    let resting = scene
        .rows
        .tiles()
        .flat_map(|(_, rows)| rows.rows())
        .filter(|row| matches!(row.state.rest, Rest::Resting(_)))
        .count();
    println!("{resting} resting rows");
    // SipHash over bytes with the fixed keys of `new` gives the same value on every target.
    let mut hasher = DefaultHasher::new();
    hasher.write(format!("{:?}\n{:?}", scene.rows, scene.grid).as_bytes());
    println!(
        "{} {threads} threads {ticks} ticks {:016x}",
        std::env::consts::ARCH,
        hasher.finish()
    );
}

//! Prints a hash of the states of the benchmark scene after a number of ticks, for a comparison
//! across thread counts and architectures:
//! `cargo run --release -p mcrs_minecraft_engine_physics --example state_hash -- <threads> <ticks>`.
#![expect(clippy::unwrap_used, reason = "a developer tool")]

use std::hash::DefaultHasher;
use std::hash::Hasher;

use bevy_tasks::ComputeTaskPool;
use bevy_tasks::TaskPoolBuilder;
use mcrs_minecraft_engine_physics::TestScene;
use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_physics::Rest;

fn main() {
    let mut arguments = std::env::args().skip(1);
    let threads: usize = arguments.next().unwrap().parse().unwrap();
    let ticks: u32 = arguments.next().unwrap().parse().unwrap();
    ComputeTaskPool::get_or_init(|| TaskPoolBuilder::new().num_threads(threads).build());
    let mut scene = TestScene::new(16, 50_000, 50_000, 11);
    for _ in 0..ticks {
        scene.step();
    }
    let states: Vec<EntityState> = scene.states().into_iter().map(|(_, state)| state).collect();
    let resting = states
        .iter()
        .filter(|state| matches!(state.rest, Rest::Resting(_)))
        .count();
    println!("{resting} resting bodies");
    // SipHash over bytes with the fixed keys of `new` gives the same value on every target. The
    // states are in key order, so the hash does not depend on the spawn order.
    let mut hasher = DefaultHasher::new();
    hasher.write(format!("{states:?}").as_bytes());
    println!(
        "{} {threads} threads {ticks} ticks {:016x}",
        std::env::consts::ARCH,
        hasher.finish()
    );
}

use mcrs_minecraft_core::Direction;
use mcrs_minecraft_simulation::{Serial, Spread};
use mcrs_voxel::BlockPos;

use crate::world::{World, above, at, east, south};

const TICKS: usize = 240;

struct Rotated {
    threads: usize,
    seed: usize,
}

impl Spread for Rotated {
    fn spread<T: Send, R: Fn(&mut T) + Sync>(&self, items: &mut [T], run: &R) {
        if items.is_empty() {
            return;
        }
        let shift = self.seed % items.len();
        items.rotate_left(shift);
        let mut parts: Vec<Vec<&mut T>> = (0..self.threads).map(|_| Vec::new()).collect();
        for (n, item) in items.iter_mut().enumerate() {
            if let Some(part) = parts.get_mut(n % self.threads) {
                part.push(item);
            }
        }
        std::thread::scope(|scope| {
            for part in parts.into_iter().filter(|part| !part.is_empty()) {
                scope.spawn(move || {
                    for item in part {
                        run(item);
                    }
                });
            }
        });
    }
}

fn build(world: &mut World) {
    let wire = world.default("minecraft:redstone_wire");
    let stone = world.default("minecraft:stone");
    let lamp = world.default("minecraft:redstone_lamp");

    let clock = at(0, -12);
    world.fill(clock, stone);
    for pos in [
        east(clock, 1),
        east(clock, 2),
        south(east(clock, 2), 1),
        south(east(clock, 2), 2),
        south(east(clock, 1), 2),
        south(clock, 2),
        south(clock, 1),
    ] {
        world.fill(pos, wire);
    }

    let line = at(10, -8);
    for step in 0..=16 {
        let pos = south(line, step);
        if step % 5 == 2 {
            let repeater = world.facing(world.default("minecraft:repeater"), Direction::North);
            world.fill(pos, world.with(repeater, "delay", "2"));
        } else {
            world.fill(pos, wire);
        }
    }
    world.fill(south(line, 17), lamp);

    let piston = world.facing(world.default("minecraft:piston"), Direction::East);
    world.fill(at(-5, 14), piston);
    for x in -4..=-1 {
        world.fill(at(x, 14), stone);
    }

    let comparator = world.facing(world.default("minecraft:comparator"), Direction::West);
    world.fill(at(-1, -18), world.default("minecraft:redstone_block"));
    world.fill(at(0, -18), world.with(comparator, "mode", "subtract"));
    world.fill(at(1, -18), lamp);
    for step in 1..=4 {
        world.fill(south(at(0, -18), step), wire);
    }

    let hopper = world.default("minecraft:hopper");
    world.fill(at(-3, 20), hopper);
    world.fill(at(-2, 20), world.default("minecraft:redstone_lamp"));
    let gate = world.default("minecraft:oak_fence_gate");
    world.fill(at(0, 20), gate);
}

fn script(world: &mut World, tick: usize) {
    let source = world.default("minecraft:redstone_block");
    let air = world.default("minecraft:air");
    let wall_torch = world.facing(
        world.default("minecraft:redstone_wall_torch"),
        Direction::East,
    );
    match tick {
        2 => world.place(east(at(0, -12), -1), wall_torch),
        5 => world.place(at(10, -9), source),
        8 => {
            let observer = world.default("minecraft:observer");
            world.place(at(-1, 8), world.facing(observer, Direction::East));
            world.place(at(0, 8), world.facing(observer, Direction::West));
        }
        10 => world.place(above(at(-1, 8)), world.default("minecraft:stone")),
        12 => world.place(above(at(-5, 14)), source),
        20 => world.place(south(at(0, -18), 5), source),
        40 => world.place(above(at(-5, 14)), air),
        60 => world.place(at(10, -9), air),
        80 => world.place(at(-1, 20), source),
        100 => world.place(south(at(0, -18), 5), air),
        _ => {}
    }
}

fn run<S: Spread>(
    spread: &S,
) -> (
    Vec<Vec<((i32, i32, i32), u16)>>,
    Vec<(BlockPos, i64)>,
    usize,
) {
    let mut world = World::new();
    build(&mut world);
    let mut history = Vec::with_capacity(TICKS);
    for tick in 0..TICKS {
        script(&mut world, tick);
        world.tick_tiled(spread);
        history.push(world.blocks());
    }
    (history, world.pending_ticks(), world.events.len())
}

#[test]
fn circuits_across_tile_boundaries_tick_the_same_on_one_thread_and_on_four() {
    let (one, one_ticks, one_events) = run(&Serial);
    let changed = one.windows(2).filter(|pair| pair[0] != pair[1]).count();
    assert!(
        changed > 50,
        "the circuits keep changing: {changed} changing ticks"
    );
    for seed in [1_usize, 7, 13] {
        let (four, four_ticks, four_events) = run(&Rotated { threads: 4, seed });
        for (tick, (a, b)) in one.iter().zip(&four).enumerate() {
            assert_eq!(a, b, "the blocks of tick {tick} with seed {seed}");
        }
        assert_eq!(one_ticks, four_ticks, "the pending ticks with seed {seed}");
        assert_eq!(
            one_events, four_events,
            "the delivered events with seed {seed}"
        );
    }
}

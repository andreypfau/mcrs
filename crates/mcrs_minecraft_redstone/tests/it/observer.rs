use mcrs_minecraft_core::Direction;

use crate::world::{World, at, east};

#[test]
fn an_observer_pulses_for_two_ticks_when_the_watched_block_changes() {
    let mut world = World::new();
    let observer = at(0, 0);
    let watched = east(observer, -1);
    let output = east(observer, 1);
    world.fill(output, world.default("minecraft:redstone_wire"));
    world.fill(
        observer,
        world.facing(world.default("minecraft:observer"), Direction::West),
    );
    world.place(watched, world.default("minecraft:stone"));
    world.ticks(2);
    assert!(!world.powered(observer), "the observer waits two ticks");
    world.tick();
    assert!(world.powered(observer));
    assert_eq!(world.power(output), 15, "the output block gets the pulse");
    world.ticks(2);
    assert!(!world.powered(observer), "the pulse ends after two ticks");
    assert_eq!(world.power(output), 0);
}

#[test]
fn an_observer_ignores_changes_beside_it() {
    let mut world = World::new();
    let observer = at(0, 0);
    world.fill(
        observer,
        world.facing(world.default("minecraft:observer"), Direction::West),
    );
    world.place(east(observer, 1), world.default("minecraft:stone"));
    world.ticks(4);
    assert!(!world.powered(observer));
}

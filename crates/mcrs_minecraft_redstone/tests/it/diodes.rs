use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_redstone::AnalogSource;
use mcrs_minecraft_simulation::UpdateRequest;
use mcrs_voxel::{BlockPos, BlockStateId};

use crate::world::{World, at, east, south};

fn repeater(world: &World, delay: u8) -> BlockStateId {
    let repeater = world.facing(world.default("minecraft:repeater"), Direction::West);
    world.with(repeater, "delay", &delay.to_string())
}

fn source(world: &World) -> BlockStateId {
    world.default("minecraft:redstone_block")
}

#[test]
fn a_repeater_delays_its_input_by_two_ticks_per_setting() {
    for delay in 1..=4_u8 {
        let mut world = World::new();
        let diode = at(0, 0);
        let input = east(diode, -1);
        let lamp = east(diode, 1);
        world.fill(lamp, world.default("minecraft:redstone_lamp"));
        world.fill(diode, repeater(&world, delay));
        world.place(input, source(&world));
        let wait = usize::from(delay) * 2;
        world.ticks(wait);
        assert!(
            !world.lit(lamp),
            "delay {delay}: the lamp waits {wait} ticks"
        );
        world.tick();
        assert!(world.powered(diode), "delay {delay}: the repeater turns on");
        assert!(
            world.lit(lamp),
            "delay {delay}: the lamp lights with the repeater"
        );
    }
}

#[test]
fn a_powered_diode_into_the_side_locks_a_repeater() {
    let mut world = World::new();
    let diode = at(0, 0);
    let lamp = east(diode, 1);
    let side = south(diode, 1);
    world.fill(lamp, world.default("minecraft:redstone_lamp"));
    world.fill(diode, repeater(&world, 1));
    let locker = world.facing(world.default("minecraft:repeater"), Direction::South);
    world.fill(side, locker);
    world.place(south(side, 1), source(&world));
    world.ticks(3);
    assert!(world.powered(side), "the side repeater is on");
    assert_eq!(world.value(world.state(diode), "locked"), "true");

    world.place(east(diode, -1), source(&world));
    world.ticks(6);
    assert!(!world.powered(diode), "a locked repeater keeps its output");
    assert!(!world.lit(lamp));

    world.place(south(side, 1), world.default("minecraft:air"));
    world.ticks(3);
    assert_eq!(world.value(world.state(diode), "locked"), "false");
    world.ticks(3);
    assert!(world.lit(lamp), "the unlocked repeater passes its input");
}

#[test]
fn a_repeater_extends_a_short_pulse_to_its_delay() {
    let mut world = World::new();
    let diode = at(0, 0);
    let input = east(diode, -1);
    let lamp = east(diode, 1);
    world.fill(lamp, world.default("minecraft:redstone_lamp"));
    world.fill(diode, repeater(&world, 2));
    world.place(input, source(&world));
    world.tick();
    world.place(input, world.default("minecraft:air"));
    world.ticks(3);
    assert!(!world.powered(diode));
    world.tick();
    assert!(
        world.powered(diode),
        "the repeater turns on four ticks after the pulse"
    );
    world.ticks(3);
    assert!(world.powered(diode), "the output lasts the delay");
    world.tick();
    assert!(!world.powered(diode), "the output ends after the delay");
}

#[test]
fn a_repeater_feeds_the_next_repeater_after_its_own_delay() {
    let mut world = World::new();
    let first = at(0, 0);
    let second = east(first, 1);
    let other = south(first, 2);
    let lamp = east(other, 1);
    world.fill(first, repeater(&world, 1));
    world.fill(second, repeater(&world, 1));
    world.fill(other, repeater(&world, 1));
    world.fill(lamp, world.default("minecraft:redstone_lamp"));
    world.place(east(first, -1), source(&world));
    world.place(east(other, -1), source(&world));
    world.ticks(3);
    assert!(world.powered(first), "the first repeater is on");
    assert!(world.powered(other), "the other repeater is on");
    assert!(
        !world.powered(second),
        "the second repeater waits its own delay"
    );
    world.ticks(2);
    assert!(
        world.powered(second),
        "the second repeater follows two ticks later"
    );
    assert!(world.lit(lamp));
}

fn comparator(world: &World, mode: &str) -> BlockStateId {
    let comparator = world.facing(world.default("minecraft:comparator"), Direction::West);
    world.with(comparator, "mode", mode)
}

#[test]
fn a_comparator_queues_its_changed_output_once() {
    let mut world = World::new();
    let diode = at(0, 0);
    let input = east(diode, -1);
    world.fill(diode, comparator(&world, "compare"));
    world.place(input, source(&world));
    world.ticks(3);
    world.place(input, world.default("minecraft:air"));
    world.ticks(3);
    world.place(input, source(&world));
    world.ticks(3);
    let queued = world
        .memory
        .comparator_dirty
        .iter()
        .filter(|pos| **pos == diode)
        .count();
    assert_eq!(
        queued, 1,
        "three output changes queue the comparator once until the host drains the list"
    );
    assert!(world.powered(diode));
}

fn side_line(world: &mut World, diode: BlockPos, length: i32) {
    let wire = world.default("minecraft:redstone_wire");
    for step in 1..=length {
        world.fill(south(diode, step), wire);
    }
    world.place(south(diode, length + 1), source(world));
}

#[test]
fn a_comparator_compares_and_subtracts_its_side_input() {
    let mut world = World::new();
    let diode = at(0, 0);
    let output = east(diode, 1);
    world.fill(output, world.default("minecraft:redstone_wire"));
    side_line(&mut world, diode, 6);
    world.place(east(diode, -1), source(&world));
    world.tick();
    assert_eq!(world.power(south(diode, 1)), 10, "the side wire carries 10");
    world.place(diode, comparator(&world, "compare"));
    world.ticks(3);
    assert_eq!(world.power(output), 15, "compare mode passes the input");

    world.place(diode, comparator(&world, "subtract"));
    world.ticks(3);
    assert_eq!(
        world.power(output),
        5,
        "subtract mode takes the side off the input"
    );
}

#[test]
fn a_side_input_above_the_input_turns_a_comparator_off() {
    let mut world = World::new();
    let diode = at(0, 0);
    let output = east(diode, 1);
    world.fill(output, world.default("minecraft:redstone_wire"));
    world.fill(south(diode, 1), source(&world));
    let wire = world.default("minecraft:redstone_wire");
    for step in 1..=5 {
        world.fill(east(diode, -step), wire);
    }
    world.place(east(diode, -6), source(&world));
    world.tick();
    assert_eq!(world.power(east(diode, -1)), 11);
    world.place(diode, comparator(&world, "compare"));
    world.ticks(3);
    assert_eq!(
        world.power(output),
        0,
        "a stronger side signal gives no output"
    );
}

#[test]
fn a_comparator_reads_an_analog_input_behind_it() {
    let mut world = World::new();
    let diode = at(0, 0);
    let chest = east(diode, -1);
    let output = east(diode, 1);
    world.fill(chest, world.default("minecraft:chest"));
    world.fill(output, world.default("minecraft:redstone_wire"));
    world
        .memory
        .analog
        .insert(chest, AnalogSource::Container(7));
    world.place(diode, comparator(&world, "compare"));
    world.ticks(3);
    assert_eq!(
        world.power(output),
        7,
        "the fill signal of the container comes out"
    );
    assert!(
        world.memory.comparator_dirty.contains(&diode),
        "a changed output is marked for the save"
    );

    world
        .memory
        .analog
        .insert(chest, AnalogSource::Container(3));
    world.request_update(UpdateRequest::Comparators {
        source: chest,
        block: Block::Chest.id(),
    });
    world.ticks(3);
    assert_eq!(
        world.power(output),
        3,
        "a comparator output update of the container refreshes it"
    );
}

#[test]
fn a_comparator_reads_an_item_frame_behind_a_solid_block() {
    let mut world = World::new();
    let diode = at(0, 0);
    let wall = east(diode, -1);
    let frame = east(diode, -2);
    let output = east(diode, 1);
    world.fill(wall, world.default("minecraft:stone"));
    world.fill(output, world.default("minecraft:redstone_wire"));
    world.memory.analog.insert(
        frame,
        AnalogSource::Frame {
            facing: Direction::West,
            signal: 5,
        },
    );
    world.place(diode, comparator(&world, "compare"));
    world.ticks(3);
    assert_eq!(
        world.power(output),
        5,
        "the frame on the far side of the block is read"
    );

    world.memory.analog.insert(
        frame,
        AnalogSource::Frame {
            facing: Direction::North,
            signal: 5,
        },
    );
    world.place(south(diode, 1), world.default("minecraft:stone"));
    world.ticks(3);
    assert_eq!(
        world.power(output),
        0,
        "a frame on another face is not read"
    );
}

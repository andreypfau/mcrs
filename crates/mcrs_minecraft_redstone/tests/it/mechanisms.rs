use mcrs_minecraft_core::Direction;
use mcrs_minecraft_redstone::{AnalogSource, BlockUse, Presence, Sensor, detector_power};
use mcrs_minecraft_simulation::{BlockFacts, UpdateRequest};
use mcrs_voxel::{BlockPos, BlockStateId};

use crate::world::{Facts, World, above, at, east, south, table};

fn source(world: &World) -> BlockStateId {
    world.default("minecraft:redstone_block")
}

fn request(world: &mut World, pos: BlockPos) {
    world.request_update(UpdateRequest::Single {
        target: pos,
        block: Facts.block_of(world.state(pos)),
    });
}

#[test]
fn power_opens_both_halves_of_an_iron_door_and_closes_them_again() {
    let mut world = World::new();
    let lower = at(0, 0);
    let upper = above(lower);
    let door = world.facing(world.default("minecraft:iron_door"), Direction::North);
    world.fill(lower, world.with(door, "half", "lower"));
    world.fill(upper, world.with(door, "half", "upper"));
    world.place(east(upper, 1), source(&world));
    world.tick();
    assert!(table().open(world.state(lower)), "the lower half opens");
    assert!(table().open(world.state(upper)), "the upper half follows");
    assert!(table().powered(world.state(lower)));
    world.place(east(upper, 1), world.default("minecraft:air"));
    world.tick();
    assert!(
        !table().open(world.state(lower)),
        "the door closes without power"
    );
    assert!(!table().open(world.state(upper)));
    assert_eq!(
        world.rig_use(lower),
        BlockUse::Unhandled,
        "an iron door does not open by hand"
    );
}

#[test]
fn a_wooden_door_opens_by_hand_and_the_other_half_follows() {
    let mut world = World::new();
    let lower = at(0, 0);
    let upper = above(lower);
    let door = world.facing(world.default("minecraft:oak_door"), Direction::North);
    world.fill(lower, world.with(door, "half", "lower"));
    world.fill(upper, world.with(door, "half", "upper"));
    let BlockUse::Toggle(toggle) = world.rig_use(upper) else {
        panic!("a wooden door opens by hand");
    };
    world.request(upper, toggle.state, toggle.flags);
    world.tick();
    assert!(table().open(world.state(upper)));
    assert!(
        table().open(world.state(lower)),
        "the lower half follows the use"
    );
}

#[test]
fn a_trapdoor_and_a_fence_gate_follow_power() {
    let mut world = World::new();
    let trapdoor = at(0, 0);
    let gate = at(0, 4);
    world.fill(trapdoor, world.default("minecraft:oak_trapdoor"));
    world.fill(gate, world.default("minecraft:oak_fence_gate"));
    world.place(east(trapdoor, 1), source(&world));
    world.place(east(gate, 1), source(&world));
    world.tick();
    assert!(table().open(world.state(trapdoor)), "the trapdoor opens");
    assert!(table().open(world.state(gate)), "the gate opens");
    world.place(east(trapdoor, 1), world.default("minecraft:air"));
    world.tick();
    assert!(!table().open(world.state(trapdoor)), "the trapdoor closes");
}

#[test]
fn powered_rails_carry_power_along_eight_rails_and_not_nine() {
    let mut world = World::new();
    let first = at(-10, 0);
    let rail = world.with(
        world.default("minecraft:powered_rail"),
        "shape",
        "east_west",
    );
    for x in 0..12 {
        world.fill(east(first, x), rail);
    }
    world.place(south(first, 1), source(&world));
    world.ticks(2);
    for x in 0..=8 {
        assert!(
            table().powered(world.state(east(first, x))),
            "rail {x} is powered through the chain"
        );
    }
    assert!(
        !table().powered(world.state(east(first, 9))),
        "the ninth rail after the source is out of reach"
    );
    world.place(south(first, 1), world.default("minecraft:air"));
    world.ticks(2);
    assert!(
        (0..12).all(|x| !table().powered(world.state(east(first, x)))),
        "the chain loses power with its source"
    );
}

#[test]
fn a_note_block_plays_on_a_rising_edge_and_on_a_use() {
    let mut world = World::new();
    let note = at(0, 0);
    world.fill(note, world.default("minecraft:note_block"));
    world.place(east(note, 1), source(&world));
    world.tick();
    assert!(table().powered(world.state(note)));
    assert_eq!(
        world
            .events
            .iter()
            .filter(|event| event.pos == note)
            .count(),
        1,
        "the rising edge plays the note once"
    );
    world.events.clear();
    world.place(east(note, 1), world.default("minecraft:air"));
    world.tick();
    assert!(!table().powered(world.state(note)));
    assert!(world.events.is_empty(), "a falling edge plays nothing");
    let BlockUse::Toggle(toggle) = world.rig_use(note) else {
        panic!("a use tunes the note block");
    };
    assert_eq!(table().note(toggle.state), 1);
    world.request(note, toggle.state, toggle.flags);
    world.tick();
    assert_eq!(
        world
            .events
            .iter()
            .filter(|event| event.pos == note)
            .count(),
        1,
        "a use plays the new note"
    );
}

#[test]
fn power_locks_a_hopper_and_triggers_a_dispenser_once() {
    let mut world = World::new();
    let hopper = at(0, 0);
    let dispenser = at(0, 4);
    world.fill(hopper, world.default("minecraft:hopper"));
    world.fill(dispenser, world.default("minecraft:dispenser"));
    assert!(table().enabled(world.state(hopper)));
    world.place(east(hopper, 1), source(&world));
    world.place(east(dispenser, 1), source(&world));
    world.tick();
    assert!(
        !table().enabled(world.state(hopper)),
        "a powered hopper is locked"
    );
    assert!(
        table().triggered(world.state(dispenser)),
        "a powered dispenser is triggered"
    );
    world.ticks(6);
    assert!(
        table().triggered(world.state(dispenser)),
        "the dispenser stays triggered while powered"
    );
    world.place(east(dispenser, 1), world.default("minecraft:air"));
    world.tick();
    assert!(
        !table().triggered(world.state(dispenser)),
        "the trigger releases with the power"
    );
}

#[test]
fn a_copper_bulb_toggles_on_each_rising_edge() {
    let mut world = World::new();
    let bulb = at(0, 0);
    world.fill(bulb, world.default("minecraft:copper_bulb"));
    assert!(!table().lit(world.state(bulb)));
    for expected in [true, false, true] {
        world.place(east(bulb, 1), source(&world));
        world.tick();
        assert_eq!(
            table().lit(world.state(bulb)),
            expected,
            "a rising edge toggles the bulb"
        );
        assert!(table().powered(world.state(bulb)));
        world.place(east(bulb, 1), world.default("minecraft:air"));
        world.tick();
        assert_eq!(
            table().lit(world.state(bulb)),
            expected,
            "a falling edge keeps the bulb"
        );
        assert!(!table().powered(world.state(bulb)));
    }
}

#[test]
fn a_tripwire_line_attaches_its_hooks_and_powers_them_while_an_entity_stands_on_it() {
    let mut world = World::new();
    let west_wall = at(0, 0);
    let east_wall = at(5, 0);
    world.fill(west_wall, world.default("minecraft:stone"));
    world.fill(east_wall, world.default("minecraft:stone"));
    let hook = world.default("minecraft:tripwire_hook");
    let string = world.default("minecraft:tripwire");
    for x in 2..=3 {
        world.place(east(west_wall, x), string);
    }
    world.tick();
    world.place(east(west_wall, 1), world.facing(hook, Direction::East));
    world.tick();
    assert!(
        !table().attached(world.state(east(west_wall, 1))),
        "a hook without a far hook is loose"
    );
    world.place(east(west_wall, 4), world.facing(hook, Direction::West));
    world.tick();
    for x in 1..=4 {
        assert!(
            table().attached(world.state(east(west_wall, x))),
            "block {x} is attached"
        );
    }
    assert!(!table().powered(world.state(east(west_wall, 1))));
    let lamp = south(east(west_wall, 1), 1);
    world.fill(lamp, world.default("minecraft:redstone_lamp"));
    let string_pos = east(west_wall, 2);
    world.snapshot.plates.insert(
        string_pos,
        Presence {
            entities: 1,
            living: 1,
        },
    );
    request(&mut world, string_pos);
    world.tick();
    assert!(
        table().powered(world.state(string_pos)),
        "the string is pressed"
    );
    assert!(
        table().powered(world.state(east(west_wall, 1))),
        "the near hook powers"
    );
    assert!(
        table().powered(world.state(east(west_wall, 4))),
        "the far hook powers"
    );
    assert!(world.lit(lamp), "the hook lights a lamp beside it");
    world.snapshot.plates.clear();
    world.ticks(12);
    assert!(
        !table().powered(world.state(east(west_wall, 1))),
        "the hook releases after its recheck"
    );
    world.ticks(5);
    assert!(!world.lit(lamp), "the lamp goes dark after its delay");
    assert_eq!(
        world.memory.sensors.get(&string_pos),
        Some(&Sensor::Tripwire)
    );
}

#[test]
fn a_daylight_detector_follows_the_sun_and_inverts_on_a_use() {
    assert_eq!(detector_power(false, 12, 15), 12);
    assert_eq!(detector_power(true, 12, 15), 0);
    assert_eq!(detector_power(true, 0, 11), 4);
    let mut world = World::new();
    let detector = at(0, 0);
    world.place(detector, world.default("minecraft:daylight_detector"));
    world.tick();
    assert_eq!(world.memory.sensors.get(&detector), Some(&Sensor::Daylight));
    world
        .snapshot
        .analog
        .insert(detector, AnalogSource::Daylight { sun: 9, sky: 15 });
    request(&mut world, detector);
    world.tick();
    assert_eq!(world.power(detector), 9, "the detector shows the sun power");
    let BlockUse::Toggle(toggle) = world.rig_use(detector) else {
        panic!("a use inverts the detector");
    };
    world.request(detector, toggle.state, toggle.flags);
    world.tick();
    assert!(table().inverted(world.state(detector)));
    assert_eq!(
        world.power(detector),
        0,
        "an inverted detector under the open sky shows 0"
    );
}

#[test]
fn a_tripwire_line_of_forty_strings_attaches_and_one_of_forty_one_does_not() {
    for (strings, attaches) in [(39, true), (40, true), (41, false)] {
        let mut world = World::new();
        let near_wall = at(-22, 0);
        world.fill(near_wall, world.default("minecraft:stone"));
        let far_wall = east(near_wall, strings + 3);
        world.fill(far_wall, world.default("minecraft:stone"));
        let hook = world.default("minecraft:tripwire_hook");
        let string = world.default("minecraft:tripwire");
        for x in 2..=strings + 1 {
            world.place(east(near_wall, x), string);
        }
        world.tick();
        world.place(east(near_wall, 1), world.facing(hook, Direction::East));
        world.place(
            east(near_wall, strings + 2),
            world.facing(hook, Direction::West),
        );
        world.ticks(2);
        assert_eq!(
            table().attached(world.state(east(near_wall, 1))),
            attaches,
            "{strings} strings between two hooks"
        );
        assert_eq!(
            table().attached(world.state(east(near_wall, strings + 2))),
            attaches,
            "the far hook with {strings} strings"
        );
    }
}

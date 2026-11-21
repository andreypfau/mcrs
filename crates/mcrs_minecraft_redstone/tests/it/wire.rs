use mcrs_minecraft_redstone::{RedstoneRules, WireEvaluator, is_cross, is_dot};

use crate::world::{World, above, at, below, east, table};

#[test]
fn fifteen_wires_light_a_lamp_and_sixteen_do_not() {
    fifteen_wires(World::new());
}

#[test]
fn the_experimental_evaluator_gives_the_same_powers() {
    fifteen_wires(World::with_rules(RedstoneRules {
        wire_evaluator: WireEvaluator::Experimental,
        ..RedstoneRules::JAVA
    }));
    let mut world = World::with_rules(RedstoneRules {
        wire_evaluator: WireEvaluator::Experimental,
        ..RedstoneRules::JAVA
    });
    let wire = world.default("minecraft:redstone_wire");
    let source = at(0, 0);
    for x in 1..=5 {
        world.fill(east(source, x), wire);
    }
    world.place(source, world.default("minecraft:redstone_block"));
    world.tick();
    world.place(source, world.default("minecraft:air"));
    world.tick();
    for x in 1..=5 {
        assert_eq!(world.power(east(source, x)), 0, "wire {x} loses its power");
    }
}

#[test]
fn the_experimental_evaluator_keeps_a_wire_at_the_edge_of_its_box_powered() {
    let mut world = World::with_rules(RedstoneRules {
        wire_evaluator: WireEvaluator::Experimental,
        ..RedstoneRules::JAVA
    });
    let wire = world.default("minecraft:redstone_wire");
    let first = at(-23, 0);
    for x in 0..=30 {
        world.fill(east(first, x), wire);
    }
    world.place(east(first, 31), world.default("minecraft:redstone_block"));
    world.ticks(4);
    world.place(east(first, -1), world.default("minecraft:redstone_block"));
    world.ticks(4);
    assert_eq!(world.power(first), 15);
    assert_eq!(
        world.power(east(first, 17)),
        2,
        "the far source reaches wire 17"
    );
    world.place(east(first, -1), world.default("minecraft:air"));
    world.ticks(4);
    assert_eq!(world.power(first), 0, "the near source is gone");
    assert_eq!(
        world.power(east(first, 17)),
        2,
        "the wire at the edge of the box keeps the power of the whole network"
    );
    assert_eq!(world.power(east(first, 16)), 1);
    assert_eq!(world.power(east(first, 15)), 0);
}

fn fifteen_wires(mut world: World) {
    let wire = world.default("minecraft:redstone_wire");
    let lamp = world.default("minecraft:redstone_lamp");
    let source = at(0, 0);
    for x in 1..=16 {
        world.fill(east(source, x), wire);
    }
    world.fill(east(source, 17), lamp);
    world.place(source, world.default("minecraft:redstone_block"));
    world.tick();
    for x in 1..=15 {
        assert_eq!(world.power(east(source, x)), 16 - x as u8, "wire {x}");
    }
    assert_eq!(
        world.power(east(source, 16)),
        0,
        "the sixteenth wire is out of reach"
    );
    assert!(
        !world.lit(east(source, 17)),
        "a lamp after 16 wires stays dark"
    );

    world.place(east(source, 16), lamp);
    world.tick();
    assert!(world.lit(east(source, 16)), "a lamp after 15 wires lights");
}

#[test]
fn wire_climbs_onto_glass_and_does_not_come_down_from_it() {
    glass_step(World::new);
    glass_step(|| {
        World::with_rules(RedstoneRules {
            wire_evaluator: WireEvaluator::Experimental,
            ..RedstoneRules::JAVA
        })
    });
}

fn glass_step(new_world: impl Fn() -> World) {
    let mut world = new_world();
    let wire = world.default("minecraft:redstone_wire");
    let low = at(0, 0);
    let glass = east(low, 1);
    let high = above(glass);
    world.fill(glass, world.default("minecraft:glass"));
    world.fill(high, wire);
    world.fill(low, wire);
    world.place(east(low, -1), world.default("minecraft:redstone_block"));
    world.tick();
    assert_eq!(world.power(low), 15);
    assert_eq!(world.power(high), 14, "power climbs onto the glass");
    assert_eq!(
        world.value(world.state(low), "east"),
        "up",
        "the low wire shows the climb"
    );

    let mut world = new_world();
    world.fill(glass, world.default("minecraft:glass"));
    world.fill(high, wire);
    world.fill(low, wire);
    world.place(east(high, 1), world.default("minecraft:redstone_block"));
    world.tick();
    assert_eq!(world.power(high), 15);
    assert_eq!(
        world.power(low),
        0,
        "power does not step down through glass"
    );
}

#[test]
fn a_lone_wire_is_a_cross_and_a_use_makes_it_a_dot() {
    let mut world = World::new();
    let wire = world.default("minecraft:redstone_wire");
    let pos = at(0, 0);
    world.place(pos, wire);
    world.tick();
    assert!(
        is_cross(table(), world.state(pos)),
        "a placed wire with no neighbour is a cross"
    );
    let toggled = world.rig_toggle(pos).expect("a cross toggles to a dot");
    assert!(is_dot(table(), toggled));
    world.place(pos, toggled);
    world.tick();
    assert!(
        is_dot(table(), world.state(pos)),
        "the dot stays while nothing connects"
    );
}

#[test]
fn a_dot_powers_only_the_block_below() {
    let mut world = World::new();
    let wire = world.default("minecraft:redstone_wire");
    let pos = at(0, 0);
    let side_lamp = east(pos, 1);
    let lamp_below = below(pos);
    world.fill(side_lamp, world.default("minecraft:redstone_lamp"));
    world.fill(lamp_below, world.default("minecraft:redstone_lamp"));
    world.fill(above(pos), world.default("minecraft:redstone_block"));
    world.place(pos, wire);
    world.tick();
    assert_eq!(world.power(pos), 15);
    assert!(world.lit(side_lamp), "a cross powers its sides");
    world.place(pos, world.rig_toggle(pos).expect("a dot"));
    world.ticks(6);
    assert!(!world.lit(side_lamp), "a dot does not power its sides");
    assert!(world.lit(lamp_below), "a dot powers the block below");
}

#[test]
fn a_wire_pops_when_its_support_goes() {
    let mut world = World::new();
    let pos = at(0, 0);
    world.place(pos, world.default("minecraft:redstone_wire"));
    world.tick();
    world.place(below(pos), world.default("minecraft:air"));
    world.tick();
    assert!(world.is(pos, "minecraft:air"), "the wire is destroyed");
}

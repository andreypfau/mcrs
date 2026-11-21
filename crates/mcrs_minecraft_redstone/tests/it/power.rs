use mcrs_voxel::{BlockPos, BlockStateId};

use crate::world::{World, above, at, east, south};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Lever {
    Open,
    Pulled,
}

struct LeverScene {
    block: BlockPos,
    lamp: BlockPos,
    wire: BlockPos,
}

fn lever_on_floor(world: &World, lever: Lever) -> BlockStateId {
    let state = world.default("minecraft:lever");
    let state = world.with(state, "face", "floor");
    let powered = match lever {
        Lever::Open => "false",
        Lever::Pulled => "true",
    };
    world.with(state, "powered", powered)
}

fn lever_scene(world: &mut World, support: &str) -> LeverScene {
    let block = at(0, 0);
    let lamp = east(block, 1);
    let wire = east(block, -1);
    world.fill(block, world.default(support));
    world.fill(lamp, world.default("minecraft:redstone_lamp"));
    world.fill(above(block), lever_on_floor(world, Lever::Open));
    world.fill(wire, world.default("minecraft:redstone_wire"));
    LeverScene { block, lamp, wire }
}

#[test]
fn a_lever_on_stone_powers_the_lamp_and_the_wire_through_the_block() {
    let mut world = World::new();
    let LeverScene { block, lamp, wire } = lever_scene(&mut world, "minecraft:stone");
    world.place(above(block), lever_on_floor(&world, Lever::Pulled));
    world.tick();
    assert!(
        world.lit(lamp),
        "the lamp beside a strongly powered block lights"
    );
    assert_eq!(
        world.power(wire),
        15,
        "the wire beside a strongly powered block takes 15"
    );
}

#[test]
fn a_lever_on_glass_powers_nothing_through_the_glass() {
    let mut world = World::new();
    let LeverScene { block, lamp, wire } = lever_scene(&mut world, "minecraft:glass");
    world.place(above(block), lever_on_floor(&world, Lever::Pulled));
    world.tick();
    assert!(!world.lit(lamp), "glass is not a conductor");
    assert_eq!(world.power(wire), 0, "glass passes no power to wire");
}

#[test]
fn weak_power_reaches_a_lamp_but_not_wire() {
    let mut world = World::new();
    let block = at(0, 0);
    let lamp = south(block, 1);
    let far_wire = east(block, 1);
    let wire = world.default("minecraft:redstone_wire");
    world.fill(block, world.default("minecraft:stone"));
    world.fill(lamp, world.default("minecraft:redstone_lamp"));
    world.fill(far_wire, wire);
    world.place(east(block, -1), wire);
    world.place(east(block, -2), wire);
    world.tick();
    assert_eq!(
        world.value(world.state(east(block, -1)), "east"),
        "side",
        "the line points into the stone block"
    );
    world.place(east(block, -3), world.default("minecraft:redstone_block"));
    world.tick();
    assert_eq!(world.power(east(block, -1)), 14);
    assert!(
        world.lit(lamp),
        "a mechanism beside a weakly powered block lights"
    );
    assert_eq!(world.power(far_wire), 0, "wire does not take weak power");
}

#[test]
fn a_redstone_torch_powers_the_block_above_strongly() {
    let mut world = World::new();
    let block = at(0, 0);
    let torch = above(block);
    let roof = above(torch);
    let wire = east(roof, 1);
    world.fill(block, world.default("minecraft:stone"));
    world.fill(roof, world.default("minecraft:stone"));
    world.fill(east(torch, 1), world.default("minecraft:stone"));
    world.fill(wire, world.default("minecraft:redstone_wire"));
    world.place(torch, world.default("minecraft:redstone_torch"));
    world.tick();
    assert_eq!(
        world.power(wire),
        15,
        "the torch strongly powers the block above"
    );
}

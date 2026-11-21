use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_redstone::RedstoneRules;
use mcrs_minecraft_simulation::UpdateRequest;
use mcrs_voxel::{BlockPos, BlockStateId};

use crate::world::{World, above, at, east, south};

#[derive(Clone, Copy, PartialEq, Eq)]
enum PistonKind {
    Normal,
    Sticky,
}

fn piston(world: &World, kind: PistonKind) -> BlockStateId {
    let block = match kind {
        PistonKind::Sticky => "minecraft:sticky_piston",
        PistonKind::Normal => "minecraft:piston",
    };
    world.facing(world.default(block), Direction::East)
}

fn stones(world: &mut World, from: BlockPos, count: i32) {
    let stone = world.default("minecraft:stone");
    for step in 0..count {
        world.fill(east(from, step), stone);
    }
}

#[test]
fn a_piston_pushes_twelve_blocks_and_not_thirteen() {
    let mut world = World::new();
    let base = at(0, 0);
    world.fill(base, piston(&world, PistonKind::Normal));
    stones(&mut world, east(base, 1), 12);
    world.place(above(base), world.default("minecraft:redstone_block"));
    world.tick();
    assert_eq!(world.value(world.state(base), "extended"), "true");
    assert!(world.is(east(base, 1), "minecraft:moving_piston"));
    world.ticks(2);
    assert!(world.is(east(base, 1), "minecraft:piston_head"));
    for step in 2..=13 {
        assert!(
            world.is(east(base, step), "minecraft:stone"),
            "stone {step}"
        );
    }

    let mut world = World::new();
    world.fill(base, piston(&world, PistonKind::Normal));
    stones(&mut world, east(base, 1), 13);
    world.place(above(base), world.default("minecraft:redstone_block"));
    world.ticks(3);
    assert_eq!(world.value(world.state(base), "extended"), "false");
    assert!(world.is(east(base, 1), "minecraft:stone"));
}

#[test]
fn a_piston_does_not_push_a_block_out_of_the_world() {
    let mut world = World::new();
    let base = BlockPos::new(0, 126, 0);
    world.fill(
        base,
        world.facing(piston(&world, PistonKind::Normal), Direction::Up),
    );
    world.fill(above(base), world.default("minecraft:stone"));
    world.place(east(base, 1), world.default("minecraft:redstone_block"));
    world.ticks(3);
    assert_eq!(
        world.value(world.state(base), "extended"),
        "false",
        "the destination above the world refuses the push"
    );
    assert!(world.is(above(base), "minecraft:stone"), "the stone stays");
}

#[test]
fn a_moving_block_without_its_record_waits_for_the_record() {
    let mut world = World::new();
    let base = at(0, 0);
    let moving = east(base, 2);
    world.fill(base, piston(&world, PistonKind::Normal));
    world.fill(east(base, 1), world.default("minecraft:stone"));
    world.place(above(base), world.default("minecraft:redstone_block"));
    world.tick();
    assert!(world.is(moving, "minecraft:moving_piston"));
    let payload = world
        .memory
        .stop_moving(moving)
        .expect("the record of the moving block");
    world.ticks(3);
    assert!(
        world.is(moving, "minecraft:moving_piston"),
        "a moving block without its record keeps its place"
    );
    world.memory.start_moving(moving, payload);
    world.request_update(UpdateRequest::Single {
        target: moving,
        block: Block::MovingPiston.id(),
    });
    world.tick();
    assert!(
        world.is(moving, "minecraft:stone"),
        "the restored record completes the move on the next update"
    );
}

#[test]
fn a_chest_stops_a_piston() {
    let mut world = World::new();
    let base = at(0, 0);
    world.fill(base, piston(&world, PistonKind::Normal));
    world.fill(east(base, 1), world.default("minecraft:chest"));
    world.place(above(base), world.default("minecraft:redstone_block"));
    world.ticks(3);
    assert_eq!(world.value(world.state(base), "extended"), "false");
}

#[test]
fn a_piston_pops_a_torch_in_its_way() {
    let mut world = World::new();
    let base = at(0, 0);
    world.fill(base, piston(&world, PistonKind::Normal));
    world.fill(east(base, 1), world.default("minecraft:torch"));
    world.place(above(base), world.default("minecraft:redstone_block"));
    world.ticks(3);
    assert_eq!(world.value(world.state(base), "extended"), "true");
    assert!(world.is(east(base, 1), "minecraft:piston_head"));
}

#[test]
fn a_sticky_piston_pulls_its_block_back() {
    let mut world = World::new();
    let base = at(0, 0);
    world.fill(base, piston(&world, PistonKind::Sticky));
    world.fill(east(base, 1), world.default("minecraft:stone"));
    world.place(above(base), world.default("minecraft:redstone_block"));
    world.ticks(3);
    assert!(world.is(east(base, 2), "minecraft:stone"));
    world.place(above(base), world.default("minecraft:air"));
    world.ticks(3);
    assert_eq!(world.value(world.state(base), "extended"), "false");
    assert!(
        world.is(east(base, 1), "minecraft:stone"),
        "the block came back"
    );
    assert!(world.is(east(base, 2), "minecraft:air"));
}

#[test]
fn a_sticky_piston_on_a_one_tick_pulse_leaves_its_block() {
    let mut world = World::new();
    let base = at(0, 0);
    world.fill(base, piston(&world, PistonKind::Sticky));
    world.fill(east(base, 1), world.default("minecraft:stone"));
    world.place(above(base), world.default("minecraft:redstone_block"));
    world.tick();
    world.place(above(base), world.default("minecraft:air"));
    world.tick();
    assert!(
        world.is(east(base, 2), "minecraft:stone"),
        "the short pulse finishes the move of the pushed stone at once"
    );
    assert!(
        world.is(base, "minecraft:moving_piston"),
        "the base retracts for two ticks"
    );
    world.ticks(2);
    assert_eq!(world.value(world.state(base), "extended"), "false");
    assert!(
        world.is(east(base, 2), "minecraft:stone"),
        "the block stays away"
    );
    assert!(
        world.is(east(base, 1), "minecraft:air"),
        "the head of the short pulse is gone"
    );
}

#[test]
fn a_sticky_piston_without_the_short_pulse_rule_pulls_its_block_back() {
    let mut world = World::with_rules(RedstoneRules {
        sticky_piston_short_pulse: false,
        ..RedstoneRules::JAVA
    });
    let base = at(0, 0);
    world.fill(base, piston(&world, PistonKind::Sticky));
    world.fill(east(base, 1), world.default("minecraft:stone"));
    world.place(above(base), world.default("minecraft:redstone_block"));
    world.tick();
    world.place(above(base), world.default("minecraft:air"));
    world.ticks(4);
    assert!(
        world.is(east(base, 1), "minecraft:stone"),
        "the block came back"
    );
}

#[test]
fn quasi_connectivity_powers_a_piston_from_above_after_an_update() {
    let mut world = World::new();
    let base = at(0, 0);
    world.fill(base, piston(&world, PistonKind::Normal));
    world.place(
        east(above(base), 1),
        world.default("minecraft:redstone_block"),
    );
    world.ticks(3);
    assert_eq!(
        world.value(world.state(base), "extended"),
        "false",
        "the piston waits for an update"
    );
    world.place(south(base, 1), world.default("minecraft:stone"));
    world.ticks(3);
    assert_eq!(world.value(world.state(base), "extended"), "true");

    let mut world = World::with_rules(RedstoneRules {
        quasi_connectivity: false,
        ..RedstoneRules::JAVA
    });
    world.fill(base, piston(&world, PistonKind::Normal));
    world.place(
        east(above(base), 1),
        world.default("minecraft:redstone_block"),
    );
    world.place(south(base, 1), world.default("minecraft:stone"));
    world.ticks(3);
    assert_eq!(world.value(world.state(base), "extended"), "false");
}

#[test]
fn slime_carries_a_block_on_its_side_and_not_honey() {
    let mut world = World::new();
    let base = above(above(at(0, 0)));
    world.fill(base, piston(&world, PistonKind::Normal));
    world.fill(east(base, 1), world.default("minecraft:slime_block"));
    world.fill(south(east(base, 1), 1), world.default("minecraft:stone"));
    world.fill(
        south(east(base, 1), -1),
        world.default("minecraft:honey_block"),
    );
    world.place(above(base), world.default("minecraft:redstone_block"));
    world.ticks(3);
    assert!(world.is(east(base, 2), "minecraft:slime_block"));
    assert!(
        world.is(south(east(base, 2), 1), "minecraft:stone"),
        "the stone moved along"
    );
    assert!(
        world.is(south(east(base, 1), -1), "minecraft:honey_block"),
        "the honey block stayed"
    );
}

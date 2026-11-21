use mcrs_minecraft_core::Direction;
use mcrs_minecraft_redstone::BlockUse;
use mcrs_minecraft_simulation::{BlockFacts, UpdateRequest};
use mcrs_voxel::{BlockPos, BlockStateId};

use crate::world::Facts;

use crate::world::{World, above, at, east, south};

fn floor_lever(world: &World, powered: bool) -> BlockStateId {
    let lever = world.with(world.default("minecraft:lever"), "face", "floor");
    world.with(lever, "powered", if powered { "true" } else { "false" })
}

#[test]
fn a_torch_inverts_two_ticks_after_its_block_changes() {
    let mut world = World::new();
    let block = at(0, 0);
    let torch = east(block, 1);
    world.fill(block, world.default("minecraft:stone"));
    world.fill(above(block), floor_lever(&world, false));
    let wall_torch = world.facing(
        world.default("minecraft:redstone_wall_torch"),
        Direction::East,
    );
    world.place(torch, wall_torch);
    world.tick();
    assert!(world.lit(torch));

    world.place(above(block), floor_lever(&world, true));
    world.ticks(2);
    assert!(world.lit(torch), "the torch waits two ticks");
    world.tick();
    assert!(!world.lit(torch), "the torch goes dark after two ticks");

    world.place(above(block), floor_lever(&world, false));
    world.ticks(2);
    assert!(!world.lit(torch));
    world.tick();
    assert!(world.lit(torch), "the torch lights again after two ticks");
}

fn press_and_watch(world: &mut World, button: &str, release_at: usize) {
    let block = at(0, 0);
    let lamp = east(block, 1);
    let press = above(block);
    world.fill(block, world.default("minecraft:stone"));
    world.fill(lamp, world.default("minecraft:redstone_lamp"));
    let unpressed = world.with(world.default(button), "face", "floor");
    world.fill(press, unpressed);
    world.place(press, world.with(unpressed, "powered", "true"));
    world.tick();
    assert!(world.lit(lamp), "the lamp lights on the press");
    world.ticks(release_at - 1);
    assert!(
        world.powered(press),
        "the button is still pressed one tick before release"
    );
    world.tick();
    assert!(!world.powered(press), "the button releases");
    world.ticks(3);
    assert!(world.lit(lamp), "the lamp waits four ticks");
    world.tick();
    assert!(!world.lit(lamp), "the lamp goes dark");
}

#[test]
fn a_stone_button_releases_after_twenty_ticks() {
    press_and_watch(&mut World::new(), "minecraft:stone_button", 20);
}

#[test]
fn a_wooden_button_releases_after_thirty_ticks() {
    press_and_watch(&mut World::new(), "minecraft:oak_button", 30);
}

#[test]
fn a_torch_clock_burns_out_and_recovers() {
    let mut world = World::new();
    let block = at(0, 0);
    let torch = east(block, 1);
    let wire = world.default("minecraft:redstone_wire");
    world.fill(block, world.default("minecraft:stone"));
    let path: [BlockPos; 6] = [
        east(block, 2),
        south(east(block, 2), 1),
        south(east(block, 2), 2),
        south(east(block, 1), 2),
        south(block, 2),
        south(block, 1),
    ];
    for pos in path {
        world.fill(pos, wire);
    }
    let wall_torch = world.facing(
        world.default("minecraft:redstone_wall_torch"),
        Direction::East,
    );
    world.place(torch, wall_torch);
    world.tick();
    let mut history = Vec::new();
    for _ in 0..400 {
        world.tick();
        history.push(world.lit(torch));
    }
    let toggles_early = history[..40]
        .windows(2)
        .filter(|pair| pair[0] != pair[1])
        .count();
    assert!(
        toggles_early >= 8,
        "the clock toggles before it burns out: {toggles_early}"
    );
    let mut dark_run = 0;
    let mut longest_dark = 0;
    let mut dark_end = 0;
    for (index, lit) in history.iter().enumerate() {
        dark_run = if *lit { 0 } else { dark_run + 1 };
        if dark_run > longest_dark {
            longest_dark = dark_run;
            dark_end = index + 1;
        }
    }
    assert!(
        longest_dark >= 150,
        "the torch stays dark for about 160 ticks after the burnout, longest dark run {longest_dark}"
    );
    assert!(
        history[dark_end..].iter().any(|lit| *lit),
        "the torch lights again after the dark interval ends at tick {dark_end}"
    );
}

#[test]
fn a_use_of_a_pressed_button_is_consumed_and_a_use_of_stone_is_not() {
    let mut world = World::new();
    let block = at(0, 0);
    let press = above(block);
    world.fill(block, world.default("minecraft:stone"));
    let button = world.with(world.default("minecraft:oak_button"), "face", "floor");
    world.fill(press, button);
    assert!(
        matches!(world.rig_use(press), BlockUse::Toggle(_)),
        "a use of a released button presses it"
    );
    world.fill(press, world.with(button, "powered", "true"));
    assert_eq!(
        world.rig_use(press),
        BlockUse::Consumed,
        "a use of a pressed button changes nothing and is still a use"
    );
    assert_eq!(
        world.rig_use(block),
        BlockUse::Unhandled,
        "stone is not a redstone block"
    );
}

#[test]
fn a_projectile_keeps_a_wooden_button_pressed_and_not_a_stone_one() {
    for (button, pressed_by_arrow) in [
        ("minecraft:oak_button", true),
        ("minecraft:stone_button", false),
    ] {
        let mut world = World::new();
        let block = at(0, 0);
        let press = above(block);
        world.fill(block, world.default("minecraft:stone"));
        world.fill(press, world.with(world.default(button), "face", "floor"));
        world.snapshot.projectiles.insert(press, 1);
        world.request_update(UpdateRequest::Single {
            target: press,
            block: Facts.block_of(world.state(press)),
        });
        world.tick();
        assert_eq!(
            world.powered(press),
            pressed_by_arrow,
            "{button} pressed by an arrow"
        );
        if !pressed_by_arrow {
            continue;
        }
        world.ticks(40);
        assert!(
            world.powered(press),
            "the button stays pressed while the arrow is inside"
        );
        world.snapshot.projectiles.clear();
        world.ticks(31);
        assert!(
            !world.powered(press),
            "the button releases at the next check"
        );
    }
}

use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_redstone::{PlateKind, Presence, plate_signal_for};
use mcrs_minecraft_simulation::UpdateRequest;
use mcrs_voxel::BlockPos;

use crate::world::{Facts, World, at, east};
use mcrs_minecraft_simulation::BlockFacts;

#[test]
fn the_signal_of_each_plate_kind_follows_its_occupants() {
    let cases = [
        (
            PlateKind::Wooden,
            Presence {
                entities: 1,
                living: 0,
            },
            15,
        ),
        (
            PlateKind::Stone,
            Presence {
                entities: 1,
                living: 0,
            },
            0,
        ),
        (
            PlateKind::Stone,
            Presence {
                entities: 2,
                living: 1,
            },
            15,
        ),
        (
            PlateKind::LightWeighted,
            Presence {
                entities: 7,
                living: 0,
            },
            7,
        ),
        (
            PlateKind::LightWeighted,
            Presence {
                entities: 40,
                living: 0,
            },
            15,
        ),
        (
            PlateKind::HeavyWeighted,
            Presence {
                entities: 1,
                living: 0,
            },
            1,
        ),
        (
            PlateKind::HeavyWeighted,
            Presence {
                entities: 10,
                living: 0,
            },
            1,
        ),
        (
            PlateKind::HeavyWeighted,
            Presence {
                entities: 11,
                living: 0,
            },
            2,
        ),
        (
            PlateKind::HeavyWeighted,
            Presence {
                entities: 150,
                living: 0,
            },
            15,
        ),
    ];
    for (kind, presence, signal) in cases {
        assert_eq!(
            plate_signal_for(kind, presence),
            signal,
            "{kind:?} with {presence:?}"
        );
    }
}

fn press(world: &mut World, plate: BlockPos, presence: Presence) {
    world.memory.plates.insert(plate, presence);
    world.request_update(UpdateRequest::Single {
        target: plate,
        block: Facts.block_of(world.state(plate)),
    });
}

#[test]
fn an_item_presses_an_oak_plate_and_it_releases_twenty_ticks_after_the_item_goes() {
    let mut world = World::new();
    let plate = at(0, 0);
    let lamp = east(plate, 1);
    world.fill(lamp, world.default("minecraft:redstone_lamp"));
    world.place(plate, world.default("minecraft:oak_pressure_plate"));
    world.tick();
    assert!(!world.powered(plate));
    let item = Presence {
        entities: 1,
        living: 0,
    };
    press(&mut world, plate, item);
    world.tick();
    assert!(world.powered(plate), "the item presses the plate");
    assert!(world.lit(lamp), "the plate powers the lamp beside it");

    world.memory.plates.clear();
    world.ticks(19);
    assert!(world.powered(plate), "the plate waits for its check");
    world.tick();
    assert!(!world.powered(plate), "the plate releases at the check");
}

#[test]
fn an_item_does_not_press_a_stone_plate() {
    let mut world = World::new();
    let plate = at(0, 0);
    world.place(plate, world.default("minecraft:stone_pressure_plate"));
    world.tick();
    press(
        &mut world,
        plate,
        Presence {
            entities: 1,
            living: 0,
        },
    );
    world.tick();
    assert!(!world.powered(plate));
    assert_eq!(
        Facts.block_of(world.state(plate)),
        Block::StonePressurePlate.id()
    );
}

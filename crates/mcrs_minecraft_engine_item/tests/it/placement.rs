use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::world::World;
use bevy_math::DVec3;
use mcrs_minecraft_engine_item::Dropped;
use mcrs_minecraft_engine_item::Given;
use mcrs_minecraft_engine_item::HolderCommandsExt;
use mcrs_minecraft_engine_item::InSlot;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_item::StackCommandsExt;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Kind;

use crate::fixture::LORED;
use crate::fixture::PLAIN;
use crate::fixture::SWORD;
use crate::fixture::app;
use crate::fixture::count;
use crate::fixture::lore;
use crate::fixture::spawn;
use crate::fixture::value;

const POS: DVec3 = DVec3::new(1.0, 2.0, 3.0);

fn holder(world: &mut World, size: usize) -> Entity {
    world.spawn(Slots::new(size)).id()
}

fn plain(world: &mut World, items: u8) -> Entity {
    spawn(world, ItemStack::new(PLAIN, count(items)))
}

fn run(world: &mut World, command: impl FnOnce(&mut World)) {
    command(world);
    world.flush();
    assert_consistent(world);
}

fn put_at(world: &mut World, stack: Entity, holder: Entity, index: u16) {
    run(world, |world| {
        world.commands().entity(stack).put_at(holder, index);
    });
}

fn slots(world: &World, holder: Entity) -> Vec<Option<Entity>> {
    world.get::<Slots>(holder).unwrap().iter().collect()
}

fn items(world: &World, stack: Entity) -> Option<u8> {
    world.get::<Count>(stack).map(|count| count.get())
}

fn given(world: &mut World) -> Vec<Given> {
    world.resource_mut::<Messages<Given>>().drain().collect()
}

/// Each slot names the stack whose `InSlot` names it, and the reverse; no stack is in a slot and
/// in the world.
fn assert_consistent(world: &mut World) {
    let mut holders = world.query::<(Entity, &Slots)>();
    for (holder, slots) in holders.iter(world) {
        for (index, stack) in slots.iter().enumerate() {
            let Some(stack) = stack else { continue };
            let slot = world.get::<InSlot>(stack).unwrap();
            assert_eq!((slot.holder(), usize::from(slot.index())), (holder, index));
        }
        assert_eq!(slots.occupied().count(), slots.iter().flatten().count());
    }
    let mut stacks = world.query::<(Entity, &InSlot, Option<&Dropped>)>();
    for (stack, slot, dropped) in stacks.iter(world) {
        let slots = world.get::<Slots>(slot.holder()).unwrap();
        assert_eq!(slots.get(usize::from(slot.index())), Some(stack));
        assert!(dropped.is_none(), "a stack has at most one placement");
    }
}

#[test]
fn put_at_takes_an_occupied_slot_and_leaves_the_other_stack_without_placement() {
    let mut app = app();
    let world = app.world_mut();
    let chest = holder(world, 3);
    let (first, second) = (plain(world, 1), plain(world, 2));
    put_at(world, first, chest, 1);
    put_at(world, second, chest, 1);
    assert_eq!(slots(world, chest), [None, Some(second), None]);
    assert!(!world.entity(first).contains::<InSlot>());
    assert_eq!(
        items(world, first),
        Some(1),
        "the evicted stack stays alive"
    );

    put_at(world, second, chest, 2);
    put_at(world, second, chest, 3);
    assert_eq!(slots(world, chest), [None, None, Some(second)]);
    assert_eq!(world.get::<Slots>(chest).unwrap().first_free(), Some(0));
}

#[test]
fn a_stack_is_in_a_slot_or_in_the_world_never_both() {
    let mut app = app();
    let world = app.world_mut();
    let chest = holder(world, 2);
    let stack = plain(world, 1);
    put_at(world, stack, chest, 0);
    run(world, |world| {
        world.commands().entity(stack).drop_at(POS);
    });
    assert_eq!(slots(world, chest), [None, None]);
    assert_eq!(world.get::<Dropped>(stack), Some(&Dropped { pos: POS }));

    put_at(world, stack, chest, 1);
    assert_eq!(slots(world, chest), [None, Some(stack)]);
    assert!(!world.entity(stack).contains::<Dropped>());
}

#[test]
fn the_despawn_of_a_stack_empties_its_slot_and_the_end_of_slots_despawns_the_stacks() {
    let mut app = app();
    let world = app.world_mut();
    let chest = holder(world, 2);
    let (kept, despawned) = (plain(world, 1), plain(world, 1));
    put_at(world, kept, chest, 0);
    put_at(world, despawned, chest, 1);
    run(world, |world| {
        world.despawn(despawned);
    });
    assert_eq!(slots(world, chest), [Some(kept), None]);
    assert_eq!(world.get::<Slots>(chest).unwrap().first_free(), Some(1));

    let ends: [fn(&mut World, Entity); 3] = [
        |world, holder| {
            world.entity_mut(holder).remove::<Slots>();
        },
        |world, holder| {
            world.entity_mut(holder).insert(Slots::new(1));
        },
        |world, holder| {
            world.despawn(holder);
        },
    ];
    for end in ends {
        let chest = holder(world, 1);
        let stack = plain(world, 1);
        put_at(world, stack, chest, 0);
        run(world, |world| end(world, chest));
        assert!(world.get_entity(stack).is_err());
    }
}

#[test]
fn swap_with_exchanges_the_placements() {
    let mut app = app();
    let world = app.world_mut();
    let (left, right) = (holder(world, 2), holder(world, 2));
    let (first, second, loose) = (plain(world, 1), plain(world, 2), plain(world, 3));
    put_at(world, first, left, 0);
    put_at(world, second, right, 1);
    let swap = |world: &mut World, stack: Entity, other: Entity| {
        run(world, |world| {
            world.commands().entity(stack).swap_with(other);
        });
    };

    swap(world, first, second);
    assert_eq!(slots(world, left), [Some(second), None]);
    assert_eq!(slots(world, right), [None, Some(first)]);

    swap(world, first, loose);
    assert_eq!(slots(world, right), [None, Some(loose)]);
    assert!(!world.entity(first).contains::<InSlot>());

    run(world, |world| {
        world.commands().entity(first).drop_at(POS);
    });
    swap(world, loose, first);
    assert_eq!(slots(world, right), [None, Some(first)]);
    assert_eq!(world.get::<Dropped>(loose), Some(&Dropped { pos: POS }));
}

#[test]
fn give_merges_into_equal_stacks_then_fills_the_first_free_slot() {
    struct Case {
        held: &'static [Option<(Kind, u8)>],
        gift: (Kind, u8),
        after: &'static [Option<u8>],
        merged_into: Option<usize>,
        remainder: Option<u8>,
    }
    let cases = [
        Case {
            held: &[Some((PLAIN, 60)), None, Some((PLAIN, 50))],
            gift: (PLAIN, 30),
            after: &[Some(64), Some(12), Some(64)],
            merged_into: Some(0),
            remainder: None,
        },
        Case {
            held: &[Some((PLAIN, 60)), None],
            gift: (PLAIN, 10),
            after: &[Some(64), Some(6)],
            merged_into: Some(0),
            remainder: None,
        },
        Case {
            held: &[Some((LORED, 10)), Some((SWORD, 1)), None],
            gift: (PLAIN, 10),
            after: &[Some(10), Some(1), Some(10)],
            merged_into: Some(2),
            remainder: None,
        },
        Case {
            held: &[Some((SWORD, 1)), None],
            gift: (SWORD, 1),
            after: &[Some(1), Some(1)],
            merged_into: Some(1),
            remainder: None,
        },
        Case {
            held: &[Some((PLAIN, 60))],
            gift: (PLAIN, 10),
            after: &[Some(64)],
            merged_into: Some(0),
            remainder: Some(6),
        },
        Case {
            held: &[Some((PLAIN, 64))],
            gift: (PLAIN, 10),
            after: &[Some(64)],
            merged_into: None,
            remainder: Some(10),
        },
    ];
    for case in cases {
        let mut app = app();
        let world = app.world_mut();
        let values = (case.held.iter())
            .map(|held| held.map(|(kind, items)| ItemStack::new(kind, count(items))))
            .collect();
        let chest = world.spawn_empty().id();
        run(world, |world| {
            world.commands().entity(chest).load_inventory(values);
        });
        let (kind, gift) = case.gift;
        run(world, |world| {
            world
                .commands()
                .entity(chest)
                .give(ItemStack::new(kind, count(gift)));
        });
        let held = slots(world, chest);
        let after: Vec<Option<u8>> = (held.iter())
            .map(|stack| stack.and_then(|stack| items(world, stack)))
            .collect();
        assert_eq!(after, case.after);
        let [given] = given(world).try_into().unwrap();
        assert_eq!(given.holder, chest);
        assert_eq!(given.stack, case.merged_into.and_then(|index| held[index]));
        let remainder = given.remainder.map(|stack| {
            assert_eq!(world.get::<InSlot>(stack), None);
            assert_eq!(world.get::<Dropped>(stack), None);
            items(world, stack).unwrap()
        });
        assert_eq!(remainder, case.remainder);
    }
}

#[test]
fn a_given_stack_entity_despawns_when_merged_and_keeps_its_placement_as_the_rest() {
    let mut app = app();
    let world = app.world_mut();
    let chest = holder(world, 1);
    let held = plain(world, 50);
    put_at(world, held, chest, 0);
    let (all, rest) = (plain(world, 4), plain(world, 20));
    run(world, |world| {
        world.commands().entity(rest).drop_at(POS);
        world.commands().entity(chest).give(all).give(rest);
    });
    assert!(world.get_entity(all).is_err());
    assert_eq!(items(world, held), Some(64));
    assert_eq!(items(world, rest), Some(10));
    assert_eq!(world.get::<Dropped>(rest), Some(&Dropped { pos: POS }));
    let remainders: Vec<Option<Entity>> =
        given(world).iter().map(|given| given.remainder).collect();
    assert_eq!(remainders, [None, Some(rest)]);

    let other = holder(world, 1);
    let into_other = plain(world, 3);
    run(world, |world| {
        world.commands().entity(into_other).put_in(other);
        world.commands().entity(rest).put_in(other);
    });
    assert_eq!(slots(world, other), [Some(into_other)]);
    assert_eq!(items(world, into_other), Some(13));
    assert!(world.get_entity(rest).is_err());
}

#[test]
fn give_or_drop_drops_the_rest_that_finds_no_room() {
    let mut app = app();
    let world = app.world_mut();
    let chest = holder(world, 1);
    let held = plain(world, 60);
    put_at(world, held, chest, 0);
    let entity = plain(world, 10);
    let other = holder(world, 1);
    put_at(world, entity, other, 0);
    run(world, |world| {
        let mut commands = world.commands();
        let mut chest = commands.entity(chest);
        chest.give_or_drop(ItemStack::new(PLAIN, count(10)), POS);
        chest.give_or_drop(entity, POS);
    });
    let remainders: Vec<Entity> = (given(world).iter())
        .map(|given| given.remainder.unwrap())
        .collect();
    assert_eq!(remainders[1], entity);
    for remainder in remainders {
        assert_eq!(world.get::<Dropped>(remainder), Some(&Dropped { pos: POS }));
        assert_eq!(world.get::<InSlot>(remainder), None);
    }
    assert_eq!(items(world, held), Some(64));
}

#[test]
fn load_inventory_spawns_the_values_in_their_slots_and_replaces_the_old_stacks() {
    let mut app = app();
    let world = app.world_mut();
    let player = holder(world, 2);
    let old = plain(world, 1);
    put_at(world, old, player, 1);
    let values = vec![
        Some(ItemStack::new(LORED, count(5)).set(lore(&["new"]))),
        None,
        Some(ItemStack::new(SWORD, count(1))),
        Some(ItemStack::new(Kind(999), count(1))),
    ];
    run(world, |world| {
        world
            .commands()
            .entity(player)
            .load_inventory(values.clone());
    });
    assert!(world.get_entity(old).is_err());
    let loaded: Vec<Option<ItemStack>> = (slots(world, player).into_iter())
        .map(|stack| stack.and_then(|stack| value(world, stack)))
        .collect();
    assert_eq!(loaded, [values[0].clone(), None, values[2].clone(), None]);
}

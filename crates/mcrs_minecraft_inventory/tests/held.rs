mod common;

use bevy_ecs::entity::Entity;
use bevy_ecs::world::World;
use mcrs_minecraft_inventory::{Op, Slot, TransactionError};
use mcrs_minecraft_item::{Held, Holds, SlotTable};

use common::{apply, holder, place, spawn, world};

fn cells(world: &World, holder: Entity) -> Vec<(u16, Entity)> {
    world.get::<SlotTable>(holder).unwrap().iter().collect()
}

#[test]
fn place_move_and_swap_keep_the_table_exact() {
    let mut world = world();
    let chest = holder(&mut world, 4);
    let a = spawn(&mut world, "stone", 1);
    let b = spawn(&mut world, "dirt", 1);
    place(&mut world, a, chest, 0).unwrap();
    place(&mut world, b, chest, 1).unwrap();
    assert_eq!(cells(&world, chest), [(0, a), (1, b)]);
    assert_eq!(world.get::<Holds>(chest).unwrap().entities(), [a, b]);

    place(&mut world, a, chest, 3).unwrap();
    assert_eq!(cells(&world, chest), [(1, b), (3, a)]);

    apply(
        &mut world,
        vec![Op::Swap {
            a: Slot::new(chest, 1),
            b: Slot::new(chest, 3),
        }],
    )
    .unwrap();
    assert_eq!(cells(&world, chest), [(1, a), (3, b)]);
    assert_eq!(world.get::<Held>(a), Some(&Held { holder: chest, index: 1 }));
    assert_eq!(world.get::<SlotTable>(chest).unwrap().first_free(0..4), Some(0));
    assert_eq!(world.get::<SlotTable>(chest).unwrap().first_free(1..2), None);
}

#[test]
fn despawning_a_stack_clears_its_cell() {
    let mut world = world();
    let chest = holder(&mut world, 2);
    let a = spawn(&mut world, "stone", 1);
    place(&mut world, a, chest, 1).unwrap();
    world.despawn(a);
    assert!(cells(&world, chest).is_empty());
    assert!(world.get::<Holds>(chest).is_none(), "an empty Holds is removed");
}

#[test]
fn despawning_a_holder_cascades_through_nested_stacks() {
    let mut world = world();
    let chest = holder(&mut world, 27);
    let shulker = spawn(&mut world, "shulker_box", 1);
    let pickaxe = spawn(&mut world, "diamond_pickaxe", 1);
    let bundle = spawn(&mut world, "bundle", 1);
    let apple = spawn(&mut world, "apple", 3);
    place(&mut world, shulker, chest, 0).unwrap();
    place(&mut world, pickaxe, shulker, 1).unwrap();
    place(&mut world, bundle, shulker, 2).unwrap();
    place(&mut world, apple, bundle, 0).unwrap();
    world.despawn(chest);
    for entity in [chest, shulker, pickaxe, bundle, apple] {
        assert!(world.get_entity(entity).is_err(), "{entity:?} survived");
    }
    assert_eq!(world.query::<&mcrs_minecraft_item::ItemStack>().iter(&world).count(), 0);
}

#[test]
fn a_list_grows_on_append_and_keeps_a_hole() {
    let mut world = world();
    let bundle = world.spawn(SlotTable::list()).id();
    let a = spawn(&mut world, "stone", 1);
    let b = spawn(&mut world, "dirt", 1);
    let c = spawn(&mut world, "apple", 1);
    assert!(matches!(
        place(&mut world, a, bundle, 1),
        Err(TransactionError::OutOfRange { index: 1, .. })
    ));
    place(&mut world, a, bundle, 0).unwrap();
    place(&mut world, b, bundle, 1).unwrap();
    place(&mut world, c, bundle, 2).unwrap();
    assert_eq!(world.get::<SlotTable>(bundle).unwrap().len(), 3);
    world.despawn(b);
    let table = world.get::<SlotTable>(bundle).unwrap();
    assert_eq!(table.len(), 3);
    assert_eq!(table.iter().collect::<Vec<_>>(), [(0, a), (2, c)]);
    assert_eq!(table.first_free(0..64), Some(1));
    let d = spawn(&mut world, "stick", 1);
    place(&mut world, d, bundle, 3).unwrap();
    assert_eq!(world.get::<SlotTable>(bundle).unwrap().len(), 4);
}

#[test]
fn a_replaced_table_is_rebuilt_from_held() {
    let mut world = world();
    let chest = holder(&mut world, 2);
    let a = spawn(&mut world, "stone", 1);
    place(&mut world, a, chest, 1).unwrap();
    world.entity_mut(chest).insert(SlotTable::fixed(3));
    assert_eq!(cells(&world, chest), [(1, a)]);
    let b = spawn(&mut world, "dirt", 1);
    assert!(matches!(
        place(&mut world, b, chest, 1),
        Err(TransactionError::Occupied { by, .. }) if by == a
    ));
}

#[test]
#[should_panic(expected = "outside the table")]
fn a_table_too_small_for_its_held_stacks_panics() {
    let mut world = world();
    let chest = holder(&mut world, 2);
    let a = spawn(&mut world, "stone", 1);
    place(&mut world, a, chest, 1).unwrap();
    world.entity_mut(chest).insert(SlotTable::fixed(1));
}

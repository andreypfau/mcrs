use crate::inventory_sync::{corpus, drain, stone, world};
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::world::World;
use mcrs_minecraft_item::{
    DirtyStacks, DroppedItem, ItemStack, SlotTable, Thrower, mutate, slots, stack_to_slot,
};
use mcrs_minecraft_protocol::item::{ContainerInput, HashedSlot, RawStack};
use mcrs_minecraft_server::world::bus::PacketPayload;
use mcrs_minecraft_server::world::item::click::{ContainerClickRequest, handle_container_clicks};
use mcrs_minecraft_server::world::item::menu::{CurrentMenu, Menu, open_menus};
use mcrs_minecraft_server::world::item::sync::sync_stack_slots;

fn opened() -> (World, Entity) {
    let (mut world, player, _anchor) = world();
    world.init_resource::<Messages<ContainerClickRequest>>();
    open_menus(&mut world);
    sync_stack_slots(&mut world);
    drain(&mut world);
    (world, player)
}

fn click(world: &mut World, player: Entity, input: ContainerInput, slot: i16, button: u8) {
    click_claiming(world, player, input, slot, button, Vec::new());
}

fn click_claiming(
    world: &mut World,
    player: Entity,
    input: ContainerInput,
    slot: i16,
    button: u8,
    changed: Vec<(u16, Option<HashedSlot>)>,
) {
    let state_id = i32::from(
        world
            .get::<Menu>(world.get::<CurrentMenu>(player).unwrap().0)
            .unwrap()
            .state_id,
    );
    world
        .resource_mut::<Messages<ContainerClickRequest>>()
        .write(ContainerClickRequest {
            player,
            container_id: 0,
            state_id,
            slot,
            button,
            input,
            changed,
            carried: None,
        });
    handle_container_clicks(world);
}

fn cell(world: &World, player: Entity, index: u16) -> Option<(Entity, u8)> {
    let stack = world.get::<SlotTable>(player).unwrap().get(index)?;
    Some((stack, world.get::<ItemStack>(stack).unwrap().count()))
}

#[test]
fn left_click_lifts_the_stack_and_puts_it_down_elsewhere() {
    let (mut world, player) = opened();
    let stack = stone(&mut world);
    mutate::move_stack(&mut world, stack, player, slots::HOTBAR.start).unwrap();

    click(
        &mut world,
        player,
        ContainerInput::Pickup,
        slots::HOTBAR.start as i16,
        0,
    );
    assert_eq!(cell(&world, player, slots::HOTBAR.start), None);
    assert_eq!(cell(&world, player, slots::CARRIED), Some((stack, 7)));

    click(
        &mut world,
        player,
        ContainerInput::Pickup,
        slots::MAIN.start as i16,
        0,
    );
    assert_eq!(cell(&world, player, slots::CARRIED), None);
    assert_eq!(cell(&world, player, slots::MAIN.start), Some((stack, 7)));
}

#[test]
fn right_click_takes_half_then_places_one() {
    let (mut world, player) = opened();
    let stack = stone(&mut world);
    mutate::move_stack(&mut world, stack, player, slots::HOTBAR.start).unwrap();

    click(
        &mut world,
        player,
        ContainerInput::Pickup,
        slots::HOTBAR.start as i16,
        1,
    );
    assert_eq!(
        cell(&world, player, slots::HOTBAR.start).map(|c| c.1),
        Some(3)
    );
    assert_eq!(cell(&world, player, slots::CARRIED).map(|c| c.1), Some(4));

    click(
        &mut world,
        player,
        ContainerInput::Pickup,
        slots::HOTBAR.start as i16,
        1,
    );
    assert_eq!(cell(&world, player, slots::HOTBAR.start), Some((stack, 4)));
    assert_eq!(cell(&world, player, slots::CARRIED).map(|c| c.1), Some(3));
}

#[test]
fn shift_click_moves_hotbar_to_main_and_swap_reaches_the_offhand() {
    let (mut world, player) = opened();
    let stack = stone(&mut world);
    mutate::move_stack(&mut world, stack, player, slots::HOTBAR.start).unwrap();

    click(
        &mut world,
        player,
        ContainerInput::QuickMove,
        slots::HOTBAR.start as i16,
        0,
    );
    assert_eq!(cell(&world, player, slots::MAIN.start), Some((stack, 7)));

    click(
        &mut world,
        player,
        ContainerInput::Swap,
        slots::MAIN.start as i16,
        40,
    );
    assert_eq!(cell(&world, player, slots::MAIN.start), None);
    assert_eq!(cell(&world, player, slots::OFFHAND), Some((stack, 7)));
}

#[test]
fn throw_turns_the_stack_into_a_dropped_item() {
    let (mut world, player) = opened();
    let stack = stone(&mut world);
    mutate::move_stack(&mut world, stack, player, slots::HOTBAR.start).unwrap();

    click(
        &mut world,
        player,
        ContainerInput::Throw,
        slots::HOTBAR.start as i16,
        1,
    );
    assert_eq!(cell(&world, player, slots::HOTBAR.start), None);
    assert!(world.get::<DroppedItem>(stack).is_some());
    assert_eq!(world.get::<Thrower>(stack), Some(&Thrower(player)));
}

#[test]
fn a_wrong_client_claim_is_corrected_and_a_stale_state_id_resends_everything() {
    let (mut world, player) = opened();
    let stack = stone(&mut world);
    mutate::move_stack(&mut world, stack, player, slots::HOTBAR.start).unwrap();
    sync_stack_slots(&mut world);
    drain(&mut world);

    let claimed = HashedSlot::create(&stack_to_slot(&world, stack, &corpus().1)).unwrap();
    let untouched = slots::MAIN.start + 3;
    click_claiming(
        &mut world,
        player,
        ContainerInput::Pickup,
        -1,
        0,
        vec![(untouched, claimed)],
    );
    assert!(
        world
            .resource::<DirtyStacks>()
            .cells
            .contains(&(player, untouched))
    );
    sync_stack_slots(&mut world);
    let packets = drain(&mut world);
    assert_eq!(packets.len(), 1, "{packets:?}");
    assert!(matches!(
        &packets[0].data,
        PacketPayload::ContainerSetSlot { slot, item, .. } if *slot == untouched as i16 && *item == RawStack::EMPTY
    ));

    world
        .resource_mut::<Messages<ContainerClickRequest>>()
        .write(ContainerClickRequest {
            player,
            container_id: 0,
            state_id: 0,
            slot: -1,
            button: 0,
            input: ContainerInput::Pickup,
            changed: Vec::new(),
            carried: None,
        });
    handle_container_clicks(&mut world);
    sync_stack_slots(&mut world);
    let packets = drain(&mut world);
    assert_eq!(packets.len(), 1, "{packets:?}");
    assert!(matches!(
        &packets[0].data,
        PacketPayload::ContainerSetContent { .. }
    ));
}

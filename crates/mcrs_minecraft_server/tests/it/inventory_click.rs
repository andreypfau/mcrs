use crate::inventory_sync::{corpus, drain, place, set_count, stone, world};
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::system::RunSystemOnce;
use bevy_ecs::world::World;
use mcrs_minecraft_inventory::{
    ContainerClickRequest, CurrentMenu, Menu, Remote, RemoteSlots, handle_container_clicks,
};
use mcrs_minecraft_item::{DroppedItem, ItemStack, SlotTable, Thrower, slots, stack_to_slot};
use mcrs_minecraft_protocol::GameMode;
use mcrs_minecraft_protocol::item::{ContainerInput, HashedStack, RawStack};
use mcrs_minecraft_server::world::bus::PacketPayload;
use mcrs_minecraft_server::world::item::click::{CloseContainerRequest, close_menus};
use mcrs_minecraft_server::world::item::menu::open_menus;
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
    changed: Vec<(u16, Option<HashedStack>)>,
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
            game_mode: GameMode::Survival,
            container_id: 0,
            state_id,
            slot,
            button,
            input,
            changed,
            carried: None,
        });
    handle_clicks(world);
}

/// A fresh reader would re-read every click of the world's lifetime, so the
/// requests are cleared once handled, as a frame's message update would.
fn handle_clicks(world: &mut World) {
    world.run_system_once(handle_container_clicks).unwrap();
    world
        .resource_mut::<Messages<ContainerClickRequest>>()
        .clear();
}

fn cell(world: &World, player: Entity, index: u16) -> Option<(Entity, u8)> {
    let stack = world.get::<SlotTable>(player).unwrap().get(index)?;
    Some((stack, world.get::<ItemStack>(stack).unwrap().count()))
}

#[test]
fn left_click_lifts_the_stack_and_puts_it_down_elsewhere() {
    let (mut world, player) = opened();
    let stack = stone(&mut world);
    place(&mut world, stack, player, slots::HOTBAR.start);

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
    place(&mut world, stack, player, slots::HOTBAR.start);

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
    place(&mut world, stack, player, slots::HOTBAR.start);

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
    place(&mut world, stack, player, slots::HOTBAR.start);

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
    place(&mut world, stack, player, slots::HOTBAR.start);
    sync_stack_slots(&mut world);
    drain(&mut world);

    let claimed = HashedStack::create(&stack_to_slot(&world, stack, &corpus().1)).unwrap();
    let untouched = slots::MAIN.start + 3;
    click_claiming(
        &mut world,
        player,
        ContainerInput::Pickup,
        -1,
        0,
        vec![(untouched, claimed)],
    );
    let menu = world.get::<CurrentMenu>(player).unwrap().0;
    assert!(matches!(
        world.get::<RemoteSlots>(menu).unwrap().cells[usize::from(untouched)],
        Remote::Claimed(Some(_))
    ));
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
            game_mode: GameMode::Survival,
            container_id: 0,
            state_id: 0,
            slot: -1,
            button: 0,
            input: ContainerInput::Pickup,
            changed: Vec::new(),
            carried: None,
        });
    handle_clicks(&mut world);
    sync_stack_slots(&mut world);
    let packets = drain(&mut world);
    assert_eq!(packets.len(), 1, "{packets:?}");
    assert!(matches!(
        &packets[0].data,
        PacketPayload::ContainerSetContent { .. }
    ));
}

#[test]
fn closing_the_menu_returns_the_carried_stack_to_the_held_slot_first() {
    let (mut world, player) = opened();
    world.init_resource::<Messages<CloseContainerRequest>>();
    let held_cell = slots::held(3);
    let held = stone(&mut world);
    set_count(&mut world, held, 30);
    place(&mut world, held, player, held_cell);
    let first = stone(&mut world);
    set_count(&mut world, first, 50);
    place(&mut world, first, player, slots::HOTBAR.start);
    let carried = stone(&mut world);
    set_count(&mut world, carried, 10);
    place(&mut world, carried, player, slots::CARRIED);

    world.write_message(CloseContainerRequest {
        player,
        container_id: 0,
    });
    close_menus(&mut world);
    assert_eq!(cell(&world, player, slots::CARRIED), None);
    assert_eq!(cell(&world, player, held_cell), Some((held, 40)));
    assert_eq!(cell(&world, player, slots::HOTBAR.start), Some((first, 50)));
}

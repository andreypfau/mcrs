use crate::inventory_sync::{drain, item, place, stack_at, stone, value, world};
use crate::support::standalone_corpus;
use bevy_ecs::change_detection::DetectChanges;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::system::RunSystemOnce;
use bevy_ecs::world::World;
use mcrs_minecraft_assets::access::RegistryAccess;
use mcrs_minecraft_inventory::{
    ContainerClickRequest, CurrentMenu, DROP_THROTTLE_LIMIT, DROP_THROTTLE_STEP, DropThrottle,
    Menu, Remote, RemoteSlots, SLOT_CLICKED_OUTSIDE, handle_container_clicks, tick_drop_throttles,
};
use mcrs_minecraft_item::{DroppedItem, Thrower, slots, stack_to_slot};
use mcrs_minecraft_protocol::GameMode;
use mcrs_minecraft_protocol::item::{
    ContainerInput, HashedStack, ProtoStack, QuickCraftButton, QuickCraftKind, QuickCraftStage,
    RawDelimitedStack, RawStack,
};
use mcrs_minecraft_registry::RegistryLookup;
use mcrs_minecraft_server::world::bus::PacketPayload;
use mcrs_minecraft_server::world::entity::player::ability::PlayerGameMode;
use mcrs_minecraft_server::world::item::click::{
    CloseContainerRequest, CreativeSlotRequest, close_menus, handle_creative_slots,
};
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

fn click(
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
    click_at(world, player, state_id, input, slot, button, changed, None);
}

fn click_at(
    world: &mut World,
    player: Entity,
    state_id: i32,
    input: ContainerInput,
    slot: i16,
    button: u8,
    changed: Vec<(u16, Option<HashedStack>)>,
    carried: Option<HashedStack>,
) {
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
            carried,
        });
    handle_clicks(world);
}

/// Queues a click without running the system, so a caller can batch several
/// packets of one drag across more than one `handle_clicks`.
fn write_click(world: &mut World, player: Entity, input: ContainerInput, slot: i16, button: u8) {
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
            changed: Vec::new(),
            carried: None,
        });
}

/// A fresh reader would re-read every click of the world's lifetime, so the
/// requests are cleared once handled, as a frame's message update would.
fn handle_clicks(world: &mut World) {
    world.run_system_once(handle_container_clicks).unwrap();
    world
        .resource_mut::<Messages<ContainerClickRequest>>()
        .clear();
}

#[test]
fn left_click_lifts_the_stack_and_puts_it_down_elsewhere() {
    let (mut world, player) = opened();
    let stack = stone(&mut world, 7);
    place(&mut world, stack, player, slots::HOTBAR.start);

    click(
        &mut world,
        player,
        ContainerInput::Pickup,
        slots::HOTBAR.start as i16,
        0,
        Vec::new(),
    );
    assert_eq!(stack_at(&world, player, slots::HOTBAR.start), None);
    assert_eq!(stack_at(&world, player, slots::CARRIED), Some((stack, 7)));

    click(
        &mut world,
        player,
        ContainerInput::Pickup,
        slots::MAIN.start as i16,
        0,
        Vec::new(),
    );
    assert_eq!(stack_at(&world, player, slots::CARRIED), None);
    assert_eq!(stack_at(&world, player, slots::MAIN.start), Some((stack, 7)));
}

#[test]
fn right_click_takes_half_then_places_one() {
    let (mut world, player) = opened();
    let stack = stone(&mut world, 7);
    place(&mut world, stack, player, slots::HOTBAR.start);

    click(
        &mut world,
        player,
        ContainerInput::Pickup,
        slots::HOTBAR.start as i16,
        1,
        Vec::new(),
    );
    assert_eq!(
        stack_at(&world, player, slots::HOTBAR.start).map(|c| c.1),
        Some(3)
    );
    assert_eq!(stack_at(&world, player, slots::CARRIED).map(|c| c.1), Some(4));

    click(
        &mut world,
        player,
        ContainerInput::Pickup,
        slots::HOTBAR.start as i16,
        1,
        Vec::new(),
    );
    assert_eq!(stack_at(&world, player, slots::HOTBAR.start), Some((stack, 4)));
    assert_eq!(stack_at(&world, player, slots::CARRIED).map(|c| c.1), Some(3));
}

#[test]
fn shift_click_moves_hotbar_to_main_and_swap_reaches_the_offhand() {
    let (mut world, player) = opened();
    let stack = stone(&mut world, 7);
    place(&mut world, stack, player, slots::HOTBAR.start);

    click(
        &mut world,
        player,
        ContainerInput::QuickMove,
        slots::HOTBAR.start as i16,
        0,
        Vec::new(),
    );
    assert_eq!(stack_at(&world, player, slots::MAIN.start), Some((stack, 7)));

    click(
        &mut world,
        player,
        ContainerInput::Swap,
        slots::MAIN.start as i16,
        40,
        Vec::new(),
    );
    assert_eq!(stack_at(&world, player, slots::MAIN.start), None);
    assert_eq!(stack_at(&world, player, slots::OFFHAND), Some((stack, 7)));
}

#[test]
fn throw_turns_the_stack_into_a_dropped_item() {
    let (mut world, player) = opened();
    let stack = stone(&mut world, 7);
    place(&mut world, stack, player, slots::HOTBAR.start);

    click(
        &mut world,
        player,
        ContainerInput::Throw,
        slots::HOTBAR.start as i16,
        1,
        Vec::new(),
    );
    assert_eq!(stack_at(&world, player, slots::HOTBAR.start), None);
    assert!(world.get::<DroppedItem>(stack).is_some());
    assert_eq!(world.get::<Thrower>(stack), Some(&Thrower(player)));
}

#[test]
fn a_click_for_a_container_the_player_no_longer_has_open_moves_nothing() {
    let (mut world, player) = opened();
    let stack = stone(&mut world, 7);
    place(&mut world, stack, player, slots::HOTBAR.start);
    sync_stack_slots(&mut world);
    drain(&mut world);

    world
        .resource_mut::<Messages<ContainerClickRequest>>()
        .write(ContainerClickRequest {
            player,
            game_mode: GameMode::Survival,
            container_id: 7,
            state_id: 0,
            slot: slots::HOTBAR.start as i16,
            button: 0,
            input: ContainerInput::Pickup,
            changed: vec![(slots::HOTBAR.start, None)],
            carried: None,
        });
    handle_clicks(&mut world);

    assert_eq!(
        stack_at(&world, player, slots::HOTBAR.start),
        Some((stack, 7))
    );
    assert_eq!(stack_at(&world, player, slots::CARRIED), None);
    let menu = world.get::<CurrentMenu>(player).unwrap().0;
    let remote = world.get::<RemoteSlots>(menu).unwrap();
    assert!(!matches!(
        remote.slots[usize::from(slots::HOTBAR.start)],
        Remote::Claimed(_)
    ));
    assert!(!matches!(remote.carried, Remote::Claimed(_)));
    sync_stack_slots(&mut world);
    let packets = drain(&mut world);
    assert!(packets.is_empty(), "{packets:?}");
}

#[test]
fn a_wrong_client_claim_is_corrected_and_a_stale_state_id_resends_everything() {
    let (mut world, player) = opened();
    let stack = stone(&mut world, 7);
    place(&mut world, stack, player, slots::HOTBAR.start);
    sync_stack_slots(&mut world);
    drain(&mut world);

    let claimed =
        HashedStack::create(&stack_to_slot(&world, stack, &standalone_corpus().1)).unwrap();
    let untouched = slots::MAIN.start + 3;
    click(
        &mut world,
        player,
        ContainerInput::Pickup,
        -1,
        0,
        vec![(untouched, claimed)],
    );
    let menu = world.get::<CurrentMenu>(player).unwrap().0;
    assert!(matches!(
        world.get::<RemoteSlots>(menu).unwrap().slots[usize::from(untouched)],
        Remote::Claimed(Some(_))
    ));
    sync_stack_slots(&mut world);
    let packets = drain(&mut world);
    assert_eq!(packets.len(), 1, "{packets:?}");
    assert!(matches!(
        &packets[0].data,
        PacketPayload::ContainerSetSlot { slot, item, .. } if *slot == untouched as i16 && *item == RawStack::EMPTY
    ));

    click_at(
        &mut world,
        player,
        0,
        ContainerInput::Pickup,
        -1,
        0,
        Vec::new(),
        None,
    );
    sync_stack_slots(&mut world);
    let packets = drain(&mut world);
    assert_eq!(packets.len(), 1, "{packets:?}");
    assert!(matches!(
        &packets[0].data,
        PacketPayload::ContainerSetContent { .. }
    ));
}

#[test]
fn a_sword_in_the_helmet_slot_is_refused_and_the_claim_is_corrected() {
    let (mut world, player) = opened();
    let sword = item(&mut world, "iron_sword", 1);
    place(&mut world, sword, player, slots::CARRIED);
    sync_stack_slots(&mut world);
    drain(&mut world);

    let claimed =
        HashedStack::create(&stack_to_slot(&world, sword, &standalone_corpus().1)).unwrap();
    click(
        &mut world,
        player,
        ContainerInput::Pickup,
        slots::ARMOR_HEAD as i16,
        0,
        vec![(slots::ARMOR_HEAD, claimed)],
    );
    assert_eq!(stack_at(&world, player, slots::CARRIED), Some((sword, 1)));
    assert_eq!(stack_at(&world, player, slots::ARMOR_HEAD), None);

    sync_stack_slots(&mut world);
    let packets = drain(&mut world);
    assert!(
        packets.iter().any(|packet| matches!(
            &packet.data,
            PacketPayload::ContainerSetSlot { slot, item, .. }
                if *slot == slots::ARMOR_HEAD as i16 && *item == RawStack::EMPTY
        )),
        "{packets:?}"
    );
    assert!(
        packets.iter().any(|packet| matches!(
            &packet.data,
            PacketPayload::SetCursorItem(item) if *item != RawStack::EMPTY
        )),
        "{packets:?}"
    );
}

#[test]
fn a_helmet_goes_into_the_helmet_slot() {
    let (mut world, player) = opened();
    let helmet = item(&mut world, "iron_helmet", 1);
    place(&mut world, helmet, player, slots::CARRIED);

    click(
        &mut world,
        player,
        ContainerInput::Pickup,
        slots::ARMOR_HEAD as i16,
        0,
        Vec::new(),
    );
    assert_eq!(stack_at(&world, player, slots::ARMOR_HEAD), Some((helmet, 1)));
    assert_eq!(stack_at(&world, player, slots::CARRIED), None);
}

#[test]
fn left_drag_splits_the_cursor_evenly_and_syncs_per_slot() {
    let (mut world, player) = opened();
    let stack = stone(&mut world, 64);
    place(&mut world, stack, player, slots::CARRIED);
    sync_stack_slots(&mut world);
    drain(&mut world);

    click(
        &mut world,
        player,
        ContainerInput::QuickCraft,
        SLOT_CLICKED_OUTSIDE,
        u8::from(QuickCraftButton {
            kind: QuickCraftKind::Split,
            stage: QuickCraftStage::Header,
        }),
        Vec::new(),
    );
    for offset in 0..5 {
        click(
            &mut world,
            player,
            ContainerInput::QuickCraft,
            slots::MAIN.start as i16 + offset,
            u8::from(QuickCraftButton {
                kind: QuickCraftKind::Split,
                stage: QuickCraftStage::Slot,
            }),
            Vec::new(),
        );
    }
    click(
        &mut world,
        player,
        ContainerInput::QuickCraft,
        SLOT_CLICKED_OUTSIDE,
        u8::from(QuickCraftButton {
            kind: QuickCraftKind::Split,
            stage: QuickCraftStage::End,
        }),
        Vec::new(),
    );

    for offset in 0..5 {
        assert_eq!(
            stack_at(&world, player, slots::MAIN.start + offset).map(|s| s.1),
            Some(12)
        );
    }
    assert_eq!(stack_at(&world, player, slots::CARRIED).map(|s| s.1), Some(4));

    sync_stack_slots(&mut world);
    let packets = drain(&mut world);
    assert!(!packets.is_empty());
    for packet in &packets {
        assert!(
            matches!(
                &packet.data,
                PacketPayload::ContainerSetSlot { .. } | PacketPayload::SetCursorItem(_)
            ),
            "{packets:?}"
        );
    }

    let menu = world.get::<CurrentMenu>(player).unwrap().0;
    assert!(world.get::<Menu>(menu).unwrap().drag.is_none());
}

#[test]
fn a_click_during_a_drag_resets_it_and_is_swallowed() {
    let (mut world, player) = opened();
    let stack = stone(&mut world, 64);
    place(&mut world, stack, player, slots::CARRIED);
    sync_stack_slots(&mut world);
    drain(&mut world);

    click(
        &mut world,
        player,
        ContainerInput::QuickCraft,
        SLOT_CLICKED_OUTSIDE,
        u8::from(QuickCraftButton {
            kind: QuickCraftKind::Split,
            stage: QuickCraftStage::Header,
        }),
        Vec::new(),
    );
    click(
        &mut world,
        player,
        ContainerInput::QuickCraft,
        9,
        u8::from(QuickCraftButton {
            kind: QuickCraftKind::Split,
            stage: QuickCraftStage::Slot,
        }),
        Vec::new(),
    );
    click(
        &mut world,
        player,
        ContainerInput::QuickCraft,
        10,
        u8::from(QuickCraftButton {
            kind: QuickCraftKind::Split,
            stage: QuickCraftStage::Slot,
        }),
        Vec::new(),
    );
    click(&mut world, player, ContainerInput::Pickup, 20, 0, Vec::new());
    click(
        &mut world,
        player,
        ContainerInput::QuickCraft,
        11,
        u8::from(QuickCraftButton {
            kind: QuickCraftKind::Split,
            stage: QuickCraftStage::Slot,
        }),
        Vec::new(),
    );
    click(
        &mut world,
        player,
        ContainerInput::QuickCraft,
        SLOT_CLICKED_OUTSIDE,
        u8::from(QuickCraftButton {
            kind: QuickCraftKind::Split,
            stage: QuickCraftStage::End,
        }),
        Vec::new(),
    );

    for slot in [9u16, 10, 11, 20] {
        assert_eq!(stack_at(&world, player, slot), None);
    }
    assert_eq!(stack_at(&world, player, slots::CARRIED).map(|s| s.1), Some(64));
    let menu = world.get::<CurrentMenu>(player).unwrap().0;
    assert!(world.get::<Menu>(menu).unwrap().drag.is_none());

    click(
        &mut world,
        player,
        ContainerInput::QuickCraft,
        SLOT_CLICKED_OUTSIDE,
        u8::from(QuickCraftButton {
            kind: QuickCraftKind::Split,
            stage: QuickCraftStage::Header,
        }),
        Vec::new(),
    );
    for slot in [9i16, 10, 11] {
        click(
            &mut world,
            player,
            ContainerInput::QuickCraft,
            slot,
            u8::from(QuickCraftButton {
                kind: QuickCraftKind::Split,
                stage: QuickCraftStage::Slot,
            }),
            Vec::new(),
        );
    }
    click(
        &mut world,
        player,
        ContainerInput::QuickCraft,
        SLOT_CLICKED_OUTSIDE,
        u8::from(QuickCraftButton {
            kind: QuickCraftKind::Split,
            stage: QuickCraftStage::End,
        }),
        Vec::new(),
    );

    for slot in [9u16, 10, 11] {
        assert_eq!(stack_at(&world, player, slot).map(|s| s.1), Some(21));
    }
    assert_eq!(stack_at(&world, player, slots::CARRIED).map(|s| s.1), Some(1));
}

#[test]
fn a_slot_or_end_packet_without_a_header_moves_nothing() {
    let (mut world, player) = opened();
    let stack = stone(&mut world, 64);
    place(&mut world, stack, player, slots::CARRIED);
    sync_stack_slots(&mut world);
    drain(&mut world);

    click(
        &mut world,
        player,
        ContainerInput::QuickCraft,
        9,
        u8::from(QuickCraftButton {
            kind: QuickCraftKind::Split,
            stage: QuickCraftStage::Slot,
        }),
        Vec::new(),
    );
    click(
        &mut world,
        player,
        ContainerInput::QuickCraft,
        SLOT_CLICKED_OUTSIDE,
        u8::from(QuickCraftButton {
            kind: QuickCraftKind::Split,
            stage: QuickCraftStage::End,
        }),
        Vec::new(),
    );

    assert_eq!(stack_at(&world, player, 9), None);
    assert_eq!(stack_at(&world, player, slots::CARRIED).map(|s| s.1), Some(64));
    let menu = world.get::<CurrentMenu>(player).unwrap().0;
    assert!(world.get::<Menu>(menu).unwrap().drag.is_none());
}

#[test]
fn a_second_header_mid_drag_resets_the_drag() {
    let (mut world, player) = opened();
    let stack = stone(&mut world, 64);
    place(&mut world, stack, player, slots::CARRIED);
    sync_stack_slots(&mut world);
    drain(&mut world);

    click(
        &mut world,
        player,
        ContainerInput::QuickCraft,
        SLOT_CLICKED_OUTSIDE,
        u8::from(QuickCraftButton {
            kind: QuickCraftKind::Split,
            stage: QuickCraftStage::Header,
        }),
        Vec::new(),
    );
    click(
        &mut world,
        player,
        ContainerInput::QuickCraft,
        9,
        u8::from(QuickCraftButton {
            kind: QuickCraftKind::Split,
            stage: QuickCraftStage::Slot,
        }),
        Vec::new(),
    );
    click(
        &mut world,
        player,
        ContainerInput::QuickCraft,
        SLOT_CLICKED_OUTSIDE,
        u8::from(QuickCraftButton {
            kind: QuickCraftKind::Split,
            stage: QuickCraftStage::Header,
        }),
        Vec::new(),
    );
    click(
        &mut world,
        player,
        ContainerInput::QuickCraft,
        10,
        u8::from(QuickCraftButton {
            kind: QuickCraftKind::Split,
            stage: QuickCraftStage::Slot,
        }),
        Vec::new(),
    );
    click(
        &mut world,
        player,
        ContainerInput::QuickCraft,
        SLOT_CLICKED_OUTSIDE,
        u8::from(QuickCraftButton {
            kind: QuickCraftKind::Split,
            stage: QuickCraftStage::End,
        }),
        Vec::new(),
    );

    assert_eq!(stack_at(&world, player, 9), None);
    assert_eq!(stack_at(&world, player, 10), None);
    assert_eq!(stack_at(&world, player, slots::CARRIED).map(|s| s.1), Some(64));
    let menu = world.get::<CurrentMenu>(player).unwrap().0;
    assert!(world.get::<Menu>(menu).unwrap().drag.is_none());
}

#[test]
fn a_header_on_an_empty_cursor_resets() {
    let (mut world, player) = opened();
    let stack = stone(&mut world, 7);
    place(&mut world, stack, player, 9);
    sync_stack_slots(&mut world);
    drain(&mut world);

    click(
        &mut world,
        player,
        ContainerInput::QuickCraft,
        SLOT_CLICKED_OUTSIDE,
        u8::from(QuickCraftButton {
            kind: QuickCraftKind::Split,
            stage: QuickCraftStage::Header,
        }),
        Vec::new(),
    );
    click(
        &mut world,
        player,
        ContainerInput::QuickCraft,
        9,
        u8::from(QuickCraftButton {
            kind: QuickCraftKind::Split,
            stage: QuickCraftStage::Slot,
        }),
        Vec::new(),
    );
    click(
        &mut world,
        player,
        ContainerInput::QuickCraft,
        SLOT_CLICKED_OUTSIDE,
        u8::from(QuickCraftButton {
            kind: QuickCraftKind::Split,
            stage: QuickCraftStage::End,
        }),
        Vec::new(),
    );

    assert_eq!(stack_at(&world, player, 9), Some((stack, 7)));
    assert_eq!(stack_at(&world, player, slots::CARRIED), None);
    let menu = world.get::<CurrentMenu>(player).unwrap().0;
    assert!(world.get::<Menu>(menu).unwrap().drag.is_none());
}

#[test]
fn an_end_right_after_a_header_or_before_a_slot_moves_nothing() {
    let (mut world, player) = opened();
    let stack = stone(&mut world, 64);
    place(&mut world, stack, player, slots::CARRIED);
    sync_stack_slots(&mut world);
    drain(&mut world);

    click(
        &mut world,
        player,
        ContainerInput::QuickCraft,
        SLOT_CLICKED_OUTSIDE,
        u8::from(QuickCraftButton {
            kind: QuickCraftKind::Split,
            stage: QuickCraftStage::Header,
        }),
        Vec::new(),
    );
    click(
        &mut world,
        player,
        ContainerInput::QuickCraft,
        SLOT_CLICKED_OUTSIDE,
        u8::from(QuickCraftButton {
            kind: QuickCraftKind::Split,
            stage: QuickCraftStage::End,
        }),
        Vec::new(),
    );

    assert_eq!(stack_at(&world, player, slots::CARRIED).map(|s| s.1), Some(64));
    let menu = world.get::<CurrentMenu>(player).unwrap().0;
    assert!(world.get::<Menu>(menu).unwrap().drag.is_none());

    click(
        &mut world,
        player,
        ContainerInput::QuickCraft,
        SLOT_CLICKED_OUTSIDE,
        u8::from(QuickCraftButton {
            kind: QuickCraftKind::Split,
            stage: QuickCraftStage::Header,
        }),
        Vec::new(),
    );
    click(
        &mut world,
        player,
        ContainerInput::QuickCraft,
        SLOT_CLICKED_OUTSIDE,
        u8::from(QuickCraftButton {
            kind: QuickCraftKind::Split,
            stage: QuickCraftStage::End,
        }),
        Vec::new(),
    );
    click(
        &mut world,
        player,
        ContainerInput::QuickCraft,
        9,
        u8::from(QuickCraftButton {
            kind: QuickCraftKind::Split,
            stage: QuickCraftStage::Slot,
        }),
        Vec::new(),
    );

    assert_eq!(stack_at(&world, player, 9), None);
    assert_eq!(stack_at(&world, player, slots::CARRIED).map(|s| s.1), Some(64));
    assert!(world.get::<Menu>(menu).unwrap().drag.is_none());
}

#[test]
fn a_full_kind_header_from_a_survival_player_resets() {
    let (mut world, player) = opened();
    let stack = stone(&mut world, 64);
    place(&mut world, stack, player, slots::CARRIED);
    sync_stack_slots(&mut world);
    drain(&mut world);

    click(
        &mut world,
        player,
        ContainerInput::QuickCraft,
        SLOT_CLICKED_OUTSIDE,
        u8::from(QuickCraftButton {
            kind: QuickCraftKind::Full,
            stage: QuickCraftStage::Header,
        }),
        Vec::new(),
    );
    click(
        &mut world,
        player,
        ContainerInput::QuickCraft,
        9,
        u8::from(QuickCraftButton {
            kind: QuickCraftKind::Full,
            stage: QuickCraftStage::Slot,
        }),
        Vec::new(),
    );
    click(
        &mut world,
        player,
        ContainerInput::QuickCraft,
        SLOT_CLICKED_OUTSIDE,
        u8::from(QuickCraftButton {
            kind: QuickCraftKind::Full,
            stage: QuickCraftStage::End,
        }),
        Vec::new(),
    );

    assert_eq!(stack_at(&world, player, 9), None);
    assert_eq!(stack_at(&world, player, slots::CARRIED).map(|s| s.1), Some(64));
    let menu = world.get::<CurrentMenu>(player).unwrap().0;
    assert!(world.get::<Menu>(menu).unwrap().drag.is_none());
}

#[test]
fn a_drag_split_across_two_ticks_still_applies() {
    let (mut world, player) = opened();
    let stack = stone(&mut world, 64);
    place(&mut world, stack, player, slots::CARRIED);
    sync_stack_slots(&mut world);
    drain(&mut world);

    write_click(
        &mut world,
        player,
        ContainerInput::QuickCraft,
        SLOT_CLICKED_OUTSIDE,
        u8::from(QuickCraftButton {
            kind: QuickCraftKind::Split,
            stage: QuickCraftStage::Header,
        }),
    );
    write_click(
        &mut world,
        player,
        ContainerInput::QuickCraft,
        9,
        u8::from(QuickCraftButton {
            kind: QuickCraftKind::Split,
            stage: QuickCraftStage::Slot,
        }),
    );
    write_click(
        &mut world,
        player,
        ContainerInput::QuickCraft,
        10,
        u8::from(QuickCraftButton {
            kind: QuickCraftKind::Split,
            stage: QuickCraftStage::Slot,
        }),
    );
    handle_clicks(&mut world);

    for slot in [11i16, 12, 13] {
        write_click(
            &mut world,
            player,
            ContainerInput::QuickCraft,
            slot,
            u8::from(QuickCraftButton {
                kind: QuickCraftKind::Split,
                stage: QuickCraftStage::Slot,
            }),
        );
    }
    write_click(
        &mut world,
        player,
        ContainerInput::QuickCraft,
        SLOT_CLICKED_OUTSIDE,
        u8::from(QuickCraftButton {
            kind: QuickCraftKind::Split,
            stage: QuickCraftStage::End,
        }),
    );
    handle_clicks(&mut world);

    for slot in 9u16..=13 {
        assert_eq!(stack_at(&world, player, slot).map(|s| s.1), Some(12));
    }
    assert_eq!(stack_at(&world, player, slots::CARRIED).map(|s| s.1), Some(4));
    let menu = world.get::<CurrentMenu>(player).unwrap().0;
    assert!(world.get::<Menu>(menu).unwrap().drag.is_none());
}

#[test]
fn a_slot_packet_outside_the_layout_is_dropped_and_the_rest_of_the_drag_applies() {
    let (mut world, player) = opened();
    let stack = stone(&mut world, 64);
    place(&mut world, stack, player, slots::CARRIED);
    sync_stack_slots(&mut world);
    drain(&mut world);

    click(
        &mut world,
        player,
        ContainerInput::QuickCraft,
        SLOT_CLICKED_OUTSIDE,
        u8::from(QuickCraftButton {
            kind: QuickCraftKind::Split,
            stage: QuickCraftStage::Header,
        }),
        Vec::new(),
    );
    click(
        &mut world,
        player,
        ContainerInput::QuickCraft,
        9,
        u8::from(QuickCraftButton {
            kind: QuickCraftKind::Split,
            stage: QuickCraftStage::Slot,
        }),
        Vec::new(),
    );
    click(
        &mut world,
        player,
        ContainerInput::QuickCraft,
        10,
        u8::from(QuickCraftButton {
            kind: QuickCraftKind::Split,
            stage: QuickCraftStage::Slot,
        }),
        Vec::new(),
    );
    click(
        &mut world,
        player,
        ContainerInput::QuickCraft,
        999,
        u8::from(QuickCraftButton {
            kind: QuickCraftKind::Split,
            stage: QuickCraftStage::Slot,
        }),
        Vec::new(),
    );
    click(
        &mut world,
        player,
        ContainerInput::QuickCraft,
        11,
        u8::from(QuickCraftButton {
            kind: QuickCraftKind::Split,
            stage: QuickCraftStage::Slot,
        }),
        Vec::new(),
    );
    click(
        &mut world,
        player,
        ContainerInput::QuickCraft,
        SLOT_CLICKED_OUTSIDE,
        u8::from(QuickCraftButton {
            kind: QuickCraftKind::Split,
            stage: QuickCraftStage::End,
        }),
        Vec::new(),
    );

    for slot in [9u16, 10, 11] {
        assert_eq!(stack_at(&world, player, slot).map(|s| s.1), Some(21));
    }
    assert_eq!(stack_at(&world, player, slots::CARRIED).map(|s| s.1), Some(1));
    let menu = world.get::<CurrentMenu>(player).unwrap().0;
    assert!(world.get::<Menu>(menu).unwrap().drag.is_none());
}

#[test]
fn a_drag_whose_claims_match_sends_no_packet() {
    let (mut world, player) = opened();
    let stack = stone(&mut world, 64);
    place(&mut world, stack, player, slots::CARRIED);
    sync_stack_slots(&mut world);
    drain(&mut world);

    click(
        &mut world,
        player,
        ContainerInput::QuickCraft,
        SLOT_CLICKED_OUTSIDE,
        u8::from(QuickCraftButton {
            kind: QuickCraftKind::Split,
            stage: QuickCraftStage::Header,
        }),
        Vec::new(),
    );
    for offset in 0i16..5 {
        click(
            &mut world,
            player,
            ContainerInput::QuickCraft,
            9 + offset,
            u8::from(QuickCraftButton {
                kind: QuickCraftKind::Split,
                stage: QuickCraftStage::Slot,
            }),
            Vec::new(),
        );
    }

    let slot_scratch = stone(&mut world, 12);
    let slot_hash =
        HashedStack::create(&stack_to_slot(&world, slot_scratch, &standalone_corpus().1)).unwrap();
    let cursor_scratch = stone(&mut world, 4);
    let cursor_hash =
        HashedStack::create(&stack_to_slot(&world, cursor_scratch, &standalone_corpus().1)).unwrap();
    let changed: Vec<(u16, Option<HashedStack>)> =
        (9u16..14).map(|slot| (slot, slot_hash.clone())).collect();

    let state_id = i32::from(
        world
            .get::<Menu>(world.get::<CurrentMenu>(player).unwrap().0)
            .unwrap()
            .state_id,
    );
    click_at(
        &mut world,
        player,
        state_id,
        ContainerInput::QuickCraft,
        SLOT_CLICKED_OUTSIDE,
        u8::from(QuickCraftButton {
            kind: QuickCraftKind::Split,
            stage: QuickCraftStage::End,
        }),
        changed,
        cursor_hash,
    );

    for slot in 9u16..14 {
        assert_eq!(stack_at(&world, player, slot).map(|s| s.1), Some(12));
    }
    assert_eq!(stack_at(&world, player, slots::CARRIED).map(|s| s.1), Some(4));

    sync_stack_slots(&mut world);
    let packets = drain(&mut world);
    assert!(packets.is_empty(), "{packets:?}");
}

#[test]
fn closing_the_menu_returns_the_carried_stack_to_the_held_slot_first() {
    let (mut world, player) = opened();
    world.init_resource::<Messages<CloseContainerRequest>>();
    let held_slot = slots::held(3);
    let held = stone(&mut world, 30);
    place(&mut world, held, player, held_slot);
    let first = stone(&mut world, 50);
    place(&mut world, first, player, slots::HOTBAR.start);
    let carried = stone(&mut world, 10);
    place(&mut world, carried, player, slots::CARRIED);

    world.write_message(CloseContainerRequest {
        player,
        container_id: 0,
    });
    close_menus(&mut world);
    assert_eq!(stack_at(&world, player, slots::CARRIED), None);
    assert_eq!(stack_at(&world, player, held_slot), Some((held, 40)));
    assert_eq!(stack_at(&world, player, slots::HOTBAR.start), Some((first, 50)));
}

#[test]
fn closing_the_menu_forgets_a_pending_drag() {
    let (mut world, player) = opened();
    world.init_resource::<Messages<CloseContainerRequest>>();
    let stack = stone(&mut world, 64);
    place(&mut world, stack, player, slots::CARRIED);
    sync_stack_slots(&mut world);
    drain(&mut world);

    click(
        &mut world,
        player,
        ContainerInput::QuickCraft,
        SLOT_CLICKED_OUTSIDE,
        u8::from(QuickCraftButton {
            kind: QuickCraftKind::Split,
            stage: QuickCraftStage::Header,
        }),
        Vec::new(),
    );
    world.write_message(CloseContainerRequest {
        player,
        container_id: 0,
    });
    close_menus(&mut world);

    let menu = world.get::<CurrentMenu>(player).unwrap().0;
    assert!(world.get::<Menu>(menu).unwrap().drag.is_none());
}

fn drop_throttle(world: &World, player: Entity) -> u32 {
    world
        .get::<DropThrottle>(player)
        .map_or(0, |throttle| throttle.0)
}

fn dropped_items(world: &mut World) -> usize {
    world.query::<&DroppedItem>().iter(world).count()
}

#[test]
fn throwing_and_clicking_outside_each_charge_the_drop_throttle() {
    let (mut world, player) = opened();
    let thrown = stone(&mut world, 7);
    place(&mut world, thrown, player, slots::HOTBAR.start);

    click(
        &mut world,
        player,
        ContainerInput::Throw,
        slots::HOTBAR.start as i16,
        1,
        Vec::new(),
    );
    assert_eq!(drop_throttle(&world, player), DROP_THROTTLE_STEP);

    let carried = stone(&mut world, 5);
    place(&mut world, carried, player, slots::CARRIED);

    click(
        &mut world,
        player,
        ContainerInput::Pickup,
        SLOT_CLICKED_OUTSIDE,
        0,
        Vec::new(),
    );
    assert_eq!(drop_throttle(&world, player), 2 * DROP_THROTTLE_STEP);
    assert_eq!(dropped_items(&mut world), 2);
}

#[test]
fn a_throw_at_the_drop_limit_is_ignored_and_the_slot_resent() {
    let (mut world, player) = opened();
    let stack = stone(&mut world, 7);
    place(&mut world, stack, player, slots::HOTBAR.start);
    sync_stack_slots(&mut world);
    drain(&mut world);
    world
        .entity_mut(player)
        .insert(DropThrottle(DROP_THROTTLE_LIMIT));

    click(
        &mut world,
        player,
        ContainerInput::Throw,
        slots::HOTBAR.start as i16,
        1,
        vec![(slots::HOTBAR.start, None)],
    );
    assert_eq!(
        stack_at(&world, player, slots::HOTBAR.start),
        Some((stack, 7))
    );
    assert_eq!(dropped_items(&mut world), 0);
    assert_eq!(drop_throttle(&world, player), DROP_THROTTLE_LIMIT);

    sync_stack_slots(&mut world);
    let packets = drain(&mut world);
    assert!(
        packets.iter().any(|packet| matches!(
            &packet.data,
            PacketPayload::ContainerSetSlot { slot, item, .. }
                if *slot == slots::HOTBAR.start as i16 && *item != RawStack::EMPTY
        )),
        "{packets:?}"
    );
}

#[test]
fn the_drop_throttle_decays_by_one_a_tick() {
    let mut world = World::new();
    let charged = world.spawn(DropThrottle(DROP_THROTTLE_STEP)).id();
    let idle = world.spawn(DropThrottle(0)).id();
    world.clear_trackers();

    world.run_system_once(tick_drop_throttles).unwrap();
    assert_eq!(
        world.get::<DropThrottle>(charged),
        Some(&DropThrottle(DROP_THROTTLE_STEP - 1))
    );
    assert_eq!(world.get::<DropThrottle>(idle), Some(&DropThrottle(0)));
    assert!(
        !world
            .entity(idle)
            .get_ref::<DropThrottle>()
            .unwrap()
            .is_changed()
    );
}

#[test]
fn a_drag_does_not_charge_the_drop_throttle() {
    let (mut world, player) = opened();
    let stack = stone(&mut world, 64);
    place(&mut world, stack, player, slots::CARRIED);
    let button = |stage| {
        u8::from(QuickCraftButton {
            kind: QuickCraftKind::Split,
            stage,
        })
    };

    click(
        &mut world,
        player,
        ContainerInput::QuickCraft,
        SLOT_CLICKED_OUTSIDE,
        button(QuickCraftStage::Header),
        Vec::new(),
    );
    for offset in 0..5 {
        click(
            &mut world,
            player,
            ContainerInput::QuickCraft,
            slots::MAIN.start as i16 + offset,
            button(QuickCraftStage::Slot),
            Vec::new(),
        );
    }
    click(
        &mut world,
        player,
        ContainerInput::QuickCraft,
        SLOT_CLICKED_OUTSIDE,
        button(QuickCraftStage::End),
        Vec::new(),
    );
    assert_eq!(
        stack_at(&world, player, slots::MAIN.start).map(|s| s.1),
        Some(12)
    );
    assert_eq!(drop_throttle(&world, player), 0);
}

#[test]
fn a_creative_drop_at_the_drop_limit_spawns_nothing() {
    let (mut world, player) = opened();
    world.init_resource::<Messages<CreativeSlotRequest>>();
    world
        .entity_mut(player)
        .insert(PlayerGameMode(GameMode::Creative));
    let registry = world.resource::<RegistryAccess>().clone();
    let stone =
        ProtoStack::from_value(&value("stone", 1), &registry as &dyn RegistryLookup).unwrap();
    let item = RawDelimitedStack::from_stack(&stone, &registry).unwrap();

    world.write_message(CreativeSlotRequest {
        player,
        slot: -1,
        item: item.clone(),
    });
    handle_creative_slots(&mut world);
    assert_eq!(dropped_items(&mut world), 1);
    assert_eq!(drop_throttle(&world, player), DROP_THROTTLE_STEP);

    world
        .entity_mut(player)
        .insert(DropThrottle(DROP_THROTTLE_LIMIT));
    world.write_message(CreativeSlotRequest {
        player,
        slot: -1,
        item,
    });
    handle_creative_slots(&mut world);
    assert_eq!(dropped_items(&mut world), 1);
    assert_eq!(drop_throttle(&world, player), DROP_THROTTLE_LIMIT);
}

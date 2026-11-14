use std::collections::BTreeMap;

use bevy_app::FixedUpdate;
use bevy_ecs::message::Messages;
use bevy_ecs::world::World;
use mcrs_minecraft_engine::bus::OutboundPlayerPacket;
use mcrs_minecraft_engine::bus::PacketPayload;
use mcrs_minecraft_engine::bus::PacketTarget;
use mcrs_minecraft_engine::persisted_registry::PersistedSnapshot;
use mcrs_minecraft_engine_item::Selected;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::session::Owner;
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_inventory_view::SlotView;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::item::ItemStackWithSlot;
use mcrs_minecraft_protocol_game::serverbound::ServerboundSetCarriedItem;
use mcrs_minecraft_vanilla_inventory::InventoryMenuSync;
use mcrs_minecraft_vanilla_item_entity::PickedUp;
use mcrs_minecraft_wire::Encode;

use crate::fixture::DIRT;
use crate::fixture::STONE;
use crate::fixture::app;
use crate::fixture::apply;
use crate::fixture::put;
use crate::fixture::restore;
use crate::fixture::saved;

const SESSION: PlayerSession = PlayerSession(7);

fn sent(world: &mut World) -> Vec<PacketPayload> {
    world
        .resource_mut::<Messages<OutboundPlayerPacket>>()
        .drain()
        .map(|packet| {
            assert!(
                matches!(packet.target, PacketTarget::SinglePlayer(SESSION)),
                "each packet goes to the restored player"
            );
            packet.data
        })
        .collect()
}

fn stack(kind: Kind, count: u8) -> Option<ItemStack> {
    Some(ItemStack::new(
        kind,
        Count::new(count).expect("a count above 0"),
    ))
}

/// INV-SEL-002: a value in `0..8` becomes the selected slot; each other value changes nothing.
#[test]
fn inv_sel_002_set_carried_item_selects_a_hotbar_slot_and_ignores_other_values() {
    let cases = [(0, 0), (8, 8), (9, 3), (u16::MAX, 3)];
    for (value, selected) in cases {
        let mut app = app();
        let world = app.world_mut();
        let mut file = PersistedSnapshot::new();
        file.put(&["SelectedItemSlot"], &3)
            .expect("the selected slot");
        let player = restore(world, &file);
        let mut body = Vec::new();
        ServerboundSetCarriedItem { slot: value }
            .encode(&mut body)
            .expect("the packet encodes");

        world.trigger(Received::<ServerboundSetCarriedItem>::new(
            player,
            body.into(),
        ));
        world.flush();
        world.run_schedule(FixedUpdate);

        assert_eq!(
            world.get::<Selected>(player),
            Some(&Selected(selected)),
            "the packet with {value}"
        );
    }
}

/// INV-SAV-003, INV-SEL-003, INV-SYN-001, INV-SYN-005: after a restore, the player gets its
/// selected slot and the 46 entries of the inventory menu with the cursor, under the state id 1,
/// and its client view is what it got. Each later restore sends again under the next state id.
#[test]
fn inv_syn_005_a_restored_player_gets_its_full_state_and_a_client_view() {
    let mut app = app();
    let world = app.world_mut();
    let inventory = vec![
        ItemStackWithSlot {
            slot: 0,
            stack: saved("stone", 5),
        },
        ItemStackWithSlot {
            slot: 9,
            stack: saved("dirt", 2),
        },
    ];
    let mut file = PersistedSnapshot::new();
    file.put(&["Inventory"], &inventory).expect("the inventory");
    file.put(&["SelectedItemSlot"], &4)
        .expect("the selected slot");
    file.put(
        &["equipment"],
        &BTreeMap::from([("head".to_owned(), saved("stone", 1))]),
    )
    .expect("the equipment");
    let player = world.spawn((Player, Owner(SESSION))).id();
    apply(world, player, &file);
    world.run_schedule(FixedUpdate);

    let packets = sent(world);
    let [
        PacketPayload::SetHeldSlot(held),
        PacketPayload::ContainerSetContent(content),
    ] = packets.as_slice()
    else {
        panic!("a set_held_slot and a container_set_content packet, not {packets:?}");
    };
    assert_eq!(held.slot.0, 4, "the selected slot of the file");
    assert_eq!(content.container_id.0, 0, "the inventory menu");
    assert_eq!(
        content.state_seqno.0, 1,
        "the first state id after the join"
    );
    let entries: Vec<(usize, u16, i32)> = content
        .slot_data
        .iter()
        .enumerate()
        .filter(|(_, stack)| !stack.is_empty())
        .map(|(index, stack)| (index, stack.id.number(), stack.count))
        .collect();
    assert_eq!(
        content.slot_data.len(),
        46,
        "each entry of the inventory menu"
    );
    assert_eq!(
        entries,
        [(5, 1, 1), (9, 2, 2), (36, 1, 5)],
        "the head, P[9] and P[0] at their menu indices"
    );
    assert!(content.carried_item.is_empty(), "the file has no cursor");

    let sync = world
        .get::<InventoryMenuSync>(player)
        .expect("a restored player has a client view");
    assert_eq!(sync.state_id().get(), 1);
    let views: Vec<Option<&SlotView>> = [0, 5, 9, 36].map(|index| sync.view().entry(index)).into();
    assert_eq!(
        views,
        [
            Some(&SlotView::Stack(None)),
            Some(&SlotView::Stack(stack(STONE, 1))),
            Some(&SlotView::Stack(stack(DIRT, 2))),
            Some(&SlotView::Stack(stack(STONE, 5))),
        ],
        "the view of each entry is the sent stack"
    );
    assert_eq!(sync.view().cursor(), &SlotView::Stack(None));

    apply(world, player, &file);
    world.run_schedule(FixedUpdate);
    let packets = sent(world);
    let content_ids: Vec<i32> = packets
        .iter()
        .filter_map(|packet| match packet {
            PacketPayload::ContainerSetContent(content) => Some(content.state_seqno.0),
            _ => None,
        })
        .collect();
    assert_eq!(
        content_ids,
        [2],
        "a later restore sends under the next state id"
    );
}

/// INV-SYN-004: a pickup into the inventory reaches the client of the collector.
#[test]
fn inv_syn_004_a_pickup_sends_the_slot_it_filled() {
    let mut app = app();
    let world = app.world_mut();
    let player = world.spawn((Player, Owner(SESSION))).id();
    apply(world, player, &PersistedSnapshot::new());
    world.run_schedule(FixedUpdate);
    sent(world);

    put(world, player, 0, STONE, 3);
    world.write_message(PickedUp {
        item: EntityId(9),
        collector: player,
        count: 3,
    });
    world.run_schedule(FixedUpdate);

    let packets = sent(world);
    let [PacketPayload::ContainerSetSlot(set)] = packets.as_slice() else {
        panic!("one container_set_slot packet, not {packets:?}");
    };
    assert_eq!(set.slot, 36, "P[0] at its menu index");
    assert_eq!((set.item.id.number(), set.item.count), (1, 3));
}

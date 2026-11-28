use std::collections::BTreeMap;

use bevy_app::FixedUpdate;
use bevy_ecs::message::Messages;
use bevy_ecs::world::World;
use mcrs_minecraft_engine::bus::OutboundPlayerPacket;
use mcrs_minecraft_engine::bus::PacketPayload;
use mcrs_minecraft_engine::bus::PacketTarget;
use mcrs_minecraft_engine::persisted_registry::PersistedSnapshot;
use mcrs_minecraft_engine_item::ItemCommandsExt;
use mcrs_minecraft_engine_item::Selected;
use mcrs_minecraft_engine_item::StackCommandsExt;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::session::Owner;
use mcrs_minecraft_engine_world::session::PlayerSession;
use mcrs_minecraft_inventory_client_view::SlotView;
use mcrs_minecraft_item_stack::BlockSet;
use mcrs_minecraft_item_stack::BlockTagId;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::CustomName;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_item_stack::Tool;
use mcrs_minecraft_item_stack::ToolRule;
use mcrs_minecraft_network::event::Received;
use mcrs_minecraft_protocol_game::item::ItemStackWithSlot;
use mcrs_minecraft_protocol_game::item::kind::ItemComponentValue;
use mcrs_minecraft_protocol_game::serverbound::ServerboundSetCarriedItem;
use mcrs_minecraft_text::Text;
use mcrs_minecraft_wire::Encode;

use crate::fixture::DIRT;
use crate::fixture::STONE;
use crate::fixture::app;
use crate::fixture::apply;
use crate::fixture::first_menu_sync;
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

#[test]
fn inv_sel_003_set_held_slot_follows_a_server_change_and_never_answers_the_client() {
    enum Change {
        WriteSelected,
        SetCarriedItem,
    }
    let cases = [
        (Change::WriteSelected, vec![6]),
        (Change::SetCarriedItem, vec![]),
    ];
    for (change, expected) in cases {
        let mut app = app();
        let world = app.world_mut();
        let player = world.spawn((Player, Owner(SESSION))).id();
        apply(world, player, &PersistedSnapshot::new());
        world.run_schedule(FixedUpdate);
        let joined = sent(world);
        assert!(
            matches!(
                joined.as_slice(),
                [
                    PacketPayload::SetHeldSlot(_),
                    PacketPayload::ContainerSetContent(_)
                ]
            ),
            "the join sends the held slot and the content, not {joined:?}"
        );
        match change {
            Change::WriteSelected => {
                *world
                    .get_mut::<Selected>(player)
                    .expect("a restored player has a selected slot") = Selected(6);
            }
            Change::SetCarriedItem => {
                let mut body = Vec::new();
                ServerboundSetCarriedItem { slot: 6 }
                    .encode(&mut body)
                    .expect("the packet encodes");
                world.trigger(Received::<ServerboundSetCarriedItem>::new(
                    player,
                    body.into(),
                ));
                world.flush();
            }
        }
        let held = |world: &mut World| -> Vec<i32> {
            world.run_schedule(FixedUpdate);
            sent(world)
                .iter()
                .map(|packet| match packet {
                    PacketPayload::SetHeldSlot(held) => held.slot.0,
                    other => panic!("only set_held_slot packets, not {other:?}"),
                })
                .collect()
        };

        assert_eq!(held(world), expected, "the tick after the change");
        assert_eq!(world.get::<Selected>(player), Some(&Selected(6)));
        assert_eq!(held(world), Vec::<i32>::new(), "the next tick");
    }
}

#[test]
fn inv_syn_005_a_joined_player_gets_its_full_state_and_a_later_restore_sends_only_the_changes() {
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

    let sync = first_menu_sync(world, player);
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

    let changed = vec![
        ItemStackWithSlot {
            slot: 0,
            stack: saved("stone", 5),
        },
        ItemStackWithSlot {
            slot: 9,
            stack: saved("stone", 7),
        },
    ];
    file.put(&["Inventory"], &changed).expect("the inventory");
    file.put(&["SelectedItemSlot"], &6)
        .expect("the selected slot");
    apply(world, player, &file);
    world.run_schedule(FixedUpdate);
    let packets = sent(world);
    let mut held = Vec::new();
    let mut slots = Vec::new();
    for packet in &packets {
        match packet {
            PacketPayload::SetHeldSlot(packet) => held.push(packet.slot.0),
            PacketPayload::ContainerSetSlot(set) => {
                slots.push((set.slot, set.item.id.number(), set.item.count));
            }
            other => panic!("only set_held_slot and container_set_slot, not {other:?}"),
        }
    }
    assert_eq!(held, [6], "the selected slot of the later restore");
    assert_eq!(slots, [(9, 1, 7)], "only the changed entry P[9]");
}

#[test]
fn inv_syn_004_a_command_write_of_a_slot_reaches_the_client_in_the_same_tick() {
    let mut app = app();
    let world = app.world_mut();
    let player = world.spawn((Player, Owner(SESSION))).id();
    apply(world, player, &PersistedSnapshot::new());
    world.run_schedule(FixedUpdate);
    sent(world);
    let slot_packets = |world: &mut World| -> Vec<(i16, u16, i32)> {
        world.run_schedule(FixedUpdate);
        sent(world)
            .iter()
            .map(|packet| match packet {
                PacketPayload::ContainerSetSlot(set) => {
                    (set.slot, set.item.id.number(), set.item.count)
                }
                other => panic!("only container_set_slot packets, not {other:?}"),
            })
            .collect()
    };

    let stack = world
        .commands()
        .spawn_stack(ItemStack::new(
            DIRT,
            Count::new(4).expect("a count above 0"),
        ))
        .put_at(player, 9)
        .id();
    world.flush();
    assert_eq!(slot_packets(world), [(9, 2, 4)], "the placed stack in P[9]");

    world.commands().entity(stack).consume(1);
    world.flush();
    assert_eq!(
        slot_packets(world),
        [(9, 2, 3)],
        "the count after the consume"
    );
    assert_eq!(slot_packets(world), [], "nothing changed");
}

#[test]
fn a_stack_without_a_wire_form_is_sent_once_per_change() {
    let mut app = app();
    let world = app.world_mut();
    let player = world.spawn((Player, Owner(SESSION))).id();
    apply(world, player, &PersistedSnapshot::new());
    world.run_schedule(FixedUpdate);
    sent(world);
    let unnamed_tag = Tool::new(vec![ToolRule::new(BlockSet::Tag(BlockTagId(u16::MAX)))]);
    world
        .commands()
        .spawn_stack(
            ItemStack::new(STONE, Count::new(1).expect("a count above 0")).set(unnamed_tag),
        )
        .put_at(player, 9);
    world.flush();

    let mut slots = Vec::new();
    for _ in 0..2 {
        world.run_schedule(FixedUpdate);
        for packet in sent(world) {
            let PacketPayload::ContainerSetSlot(set) = packet else {
                panic!("only container_set_slot packets, not {packet:?}");
            };
            slots.push((set.slot, set.item.is_empty()));
        }
    }

    assert_eq!(slots, [(9, true)], "one empty stack in P[9] in two ticks");
}

#[test]
fn inv_syn_003_a_stack_with_a_patch_is_sent_with_its_patch() {
    let mut app = app();
    let world = app.world_mut();
    let player = world.spawn((Player, Owner(SESSION))).id();
    apply(world, player, &PersistedSnapshot::new());
    world.run_schedule(FixedUpdate);
    sent(world);
    let named = ItemStack::new(STONE, Count::new(3).expect("a count above 0"))
        .set(CustomName(Text::text("Rock")));
    world.commands().spawn_stack(named).put_at(player, 9);
    world.flush();

    world.run_schedule(FixedUpdate);
    let packets = sent(world);

    let [PacketPayload::ContainerSetSlot(set)] = packets.as_slice() else {
        panic!("one container_set_slot packet, not {packets:?}");
    };
    assert_eq!(
        (set.slot, set.item.id.number(), set.item.count),
        (9, 1, 3),
        "the named stone in P[9]"
    );
    assert!(
        matches!(
            set.item.components.added.as_slice(),
            [ItemComponentValue::CustomName(_)]
        ),
        "the patch holds the custom name, not {:?}",
        set.item.components
    );
    assert!(
        set.item.components.removed.is_empty(),
        "the patch removes nothing"
    );
}

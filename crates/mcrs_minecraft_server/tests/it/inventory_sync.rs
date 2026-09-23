use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::system::Command;
use bevy_ecs::world::World;
use mcrs_minecraft_assets::{RegistryAccess, RegistrySnapshotErased};
use mcrs_minecraft_core::codec::Bounded;
use mcrs_minecraft_core::{ResourceKey, ResourceLocation};
use mcrs_minecraft_inventory::value::spawn_stack;
use mcrs_minecraft_inventory::{CurrentMenu, Menu, Op, Slot, Transaction};
use mcrs_minecraft_item::{ItemStack, SelectedHotbarSlot, SlotTable, slots, stack_to_value};
use mcrs_minecraft_level::entity::physics::Transform;
use mcrs_minecraft_level::entity::player::Player;
use mcrs_minecraft_level::world::dimension::InDimension;
use mcrs_minecraft_protocol::item::{ComponentPatch, ItemStackValue, RawStack};
use mcrs_minecraft_server::world::bus::{OutboundPlayerPacket, PacketPayload, PacketTarget};
use mcrs_minecraft_server::world::entity::player::HostAnchor;
use mcrs_minecraft_server::world::item::chest::{
    OpenContainerRequest, close_container_menu, open_containers,
};
use mcrs_minecraft_server::world::item::menu::open_menus;
use mcrs_minecraft_server::world::item::sync::sync_stack_slots;

use crate::support::standalone_corpus;

pub(crate) fn world() -> (World, Entity, Entity) {
    let (blocks, items) = standalone_corpus();
    let mut registry = RegistryAccess::default();
    registry.register(Box::new(RegistrySnapshotErased::from_entries(
        "minecraft:item",
        items
            .0
            .iter()
            .map(|entry| (entry.identifier.clone(), None))
            .collect(),
        None,
    )));
    let mut world = World::new();
    world.insert_resource(blocks.clone());
    world.insert_resource(items.clone());
    world.insert_resource(registry);
    world.init_resource::<Messages<OutboundPlayerPacket>>();
    let anchor = world.spawn_empty().id();
    let player = world
        .spawn((
            Player,
            SlotTable::fixed(slots::COUNT),
            SelectedHotbarSlot(3),
            HostAnchor(anchor),
            InDimension(anchor),
            Transform::IDENTITY,
        ))
        .id();
    (world, player, anchor)
}

pub(crate) fn drain(world: &mut World) -> Vec<OutboundPlayerPacket> {
    world
        .resource_mut::<Messages<OutboundPlayerPacket>>()
        .drain()
        .collect()
}

pub(crate) fn value(item: &str, count: u8) -> ItemStackValue {
    ItemStackValue {
        item: ResourceKey::from_location(ResourceLocation::minecraft(item)),
        count: Bounded(i32::from(count)),
        components: ComponentPatch::EMPTY,
    }
}

pub(crate) fn item(world: &mut World, path: &str, count: u8) -> Entity {
    spawn_stack(world, &value(path, count), &standalone_corpus().1).unwrap()
}

pub(crate) fn stone(world: &mut World, count: u8) -> Entity {
    item(world, "stone", count)
}

pub(crate) fn stack_at(world: &World, player: Entity, index: u16) -> Option<(Entity, u8)> {
    let stack = world.get::<SlotTable>(player).unwrap().get(index)?;
    Some((stack, world.get::<ItemStack>(stack).unwrap().count))
}

pub(crate) fn place(world: &mut World, stack: Entity, holder: Entity, index: u16) {
    Transaction(vec![Op::Place {
        stack,
        to: Slot::new(holder, index),
    }])
    .apply(world);
}

pub(crate) fn set_count(world: &mut World, stack: Entity, count: u8) {
    let op = match count {
        0 => Op::Despawn { stack },
        count => {
            let mut value = stack_to_value(world, stack, &standalone_corpus().1);
            value.count = Bounded(i32::from(count));
            Op::Apply { stack, value }
        }
    };
    Transaction(vec![op]).apply(world);
}

#[test]
fn join_sends_held_slot_then_full_inventory() {
    let (mut world, _player, anchor) = world();
    open_menus(&mut world);
    sync_stack_slots(&mut world);
    let packets = drain(&mut world);
    assert_eq!(packets.len(), 2, "{packets:?}");
    for packet in &packets {
        assert!(matches!(packet.target, PacketTarget::SinglePlayer(a) if a == anchor));
    }
    assert!(matches!(packets[0].data, PacketPayload::SetHeldSlot(3)));
    match &packets[1].data {
        PacketPayload::ContainerSetContent {
            container_id,
            state_id,
            slots,
            carried,
        } => {
            assert_eq!(
                (*container_id, *state_id, slots.len()),
                (0, 1, slots::MENU_COUNT)
            );
            assert!(slots.iter().all(|slot| *slot == RawStack::EMPTY));
            assert_eq!(*carried, RawStack::EMPTY);
        }
        other => panic!("{other:?}"),
    }

    open_menus(&mut world);
    sync_stack_slots(&mut world);
    assert!(
        drain(&mut world).is_empty(),
        "the inventory menu opens once"
    );
}

#[test]
fn dirty_slots_become_set_slot_and_cursor_packets() {
    let (mut world, player, _anchor) = world();
    open_menus(&mut world);
    sync_stack_slots(&mut world);
    drain(&mut world);

    let stack = stone(&mut world, 7);
    place(&mut world, stack, player, slots::HOTBAR.start);
    sync_stack_slots(&mut world);
    let packets = drain(&mut world);
    assert_eq!(packets.len(), 1, "{packets:?}");
    let PacketPayload::ContainerSetSlot {
        container_id,
        state_id,
        slot,
        item,
    } = &packets[0].data
    else {
        panic!("{packets:?}");
    };
    assert_eq!(
        (*container_id, *state_id, *slot),
        (0, 2, slots::HOTBAR.start as i16)
    );
    assert_ne!(*item, RawStack::EMPTY);

    place(&mut world, stack, player, slots::CARRIED);
    sync_stack_slots(&mut world);
    let packets = drain(&mut world);
    assert_eq!(packets.len(), 2, "{packets:?}");
    assert!(matches!(
        &packets[0].data,
        PacketPayload::ContainerSetSlot { state_id: 3, slot, item, .. }
            if *slot == slots::HOTBAR.start as i16 && *item == RawStack::EMPTY
    ));
    assert!(
        matches!(&packets[1].data, PacketPayload::SetCursorItem(item) if *item != RawStack::EMPTY)
    );

    sync_stack_slots(&mut world);
    assert!(drain(&mut world).is_empty());
}

/// Runs the sync and starts a new change-detection tick, as the schedule does
/// between two frames.
fn sync_tick(world: &mut World) -> Vec<OutboundPlayerPacket> {
    sync_stack_slots(world);
    world.clear_trackers();
    drain(world)
}

fn open_chest(world: &mut World, player: Entity) -> Entity {
    world.init_resource::<Messages<OpenContainerRequest>>();
    let chest = world.spawn(SlotTable::fixed(27)).id();
    world.write_message(OpenContainerRequest {
        player,
        container: chest,
    });
    open_containers(world);
    sync_tick(world);
    world.get::<CurrentMenu>(player).unwrap().0
}

#[test]
fn a_slot_outside_the_chest_layout_that_changed_is_resent_when_the_chest_closes() {
    let (mut world, player, _anchor) = world();
    open_menus(&mut world);
    sync_tick(&mut world);
    let chest_menu = open_chest(&mut world, player);

    let helmet = item(&mut world, "iron_helmet", 1);
    place(&mut world, helmet, player, slots::ARMOR_HEAD);
    let packets = sync_tick(&mut world);
    assert!(packets.is_empty(), "{packets:?}");

    close_container_menu(&mut world, player, chest_menu, false);
    let packets = sync_tick(&mut world);
    assert!(
        !packets
            .iter()
            .any(|packet| matches!(packet.data, PacketPayload::ContainerSetContent { .. })),
        "{packets:?}"
    );
    assert!(
        packets.iter().any(|packet| matches!(
            &packet.data,
            PacketPayload::ContainerSetSlot { container_id: 0, slot, item, .. }
                if *slot == slots::ARMOR_HEAD as i16 && *item != RawStack::EMPTY
        )),
        "{packets:?}"
    );
}

#[test]
fn a_helmet_equipped_before_a_chest_opens_survives_the_close_without_a_full_resend() {
    let (mut world, player, _anchor) = world();
    open_menus(&mut world);
    sync_tick(&mut world);
    let helmet = item(&mut world, "iron_helmet", 1);
    place(&mut world, helmet, player, slots::ARMOR_HEAD);
    sync_tick(&mut world);
    let inventory_menu = world.get::<CurrentMenu>(player).unwrap().0;
    let state_id = world.get::<Menu>(inventory_menu).unwrap().state_id;

    let chest_menu = open_chest(&mut world, player);
    close_container_menu(&mut world, player, chest_menu, false);
    let packets = sync_tick(&mut world);

    assert!(packets.is_empty(), "{packets:?}");
    assert_eq!(world.get::<CurrentMenu>(player).unwrap().0, inventory_menu);
    assert_eq!(
        world.get::<Menu>(inventory_menu).unwrap().state_id,
        state_id
    );
    assert_eq!(
        stack_at(&world, player, slots::ARMOR_HEAD),
        Some((helmet, 1))
    );
}

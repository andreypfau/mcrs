use std::sync::{Arc, OnceLock};

use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::{AssetPlugin, AssetServer};
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::world::World;
use mcrs_minecraft_assets::{RegistryAccess, RegistrySnapshotErased};
use mcrs_minecraft_block::definition::{Blocks, load_block_definitions};
use mcrs_minecraft_core::codec::Bounded;
use mcrs_minecraft_core::{ResourceKey, ResourceLocation};
use mcrs_minecraft_item::{
    DirtyStacks, Items, SelectedHotbarSlot, SlotTable, load_item_definitions, mutate, slots,
};
use mcrs_minecraft_level::entity::player::Player;
use mcrs_minecraft_protocol::item::{ComponentPatch, ItemStackValue, RawStack};
use mcrs_minecraft_server::world::bus::{OutboundPlayerPacket, PacketPayload, PacketTarget};
use mcrs_minecraft_server::world::entity::player::HostAnchor;
use mcrs_minecraft_server::world::item::menu::open_menus;
use mcrs_minecraft_server::world::item::sync::{MenuResync, sync_stack_slots};

pub(crate) fn corpus() -> &'static (Blocks, Items) {
    static CORPUS: OnceLock<(Blocks, Items)> = OnceLock::new();
    CORPUS.get_or_init(|| {
        let mut app = App::new();
        app.add_plugins(TaskPoolPlugin::default());
        app.add_plugins(AssetPlugin {
            watch_for_changes_override: Some(false),
            ..Default::default()
        });
        let asset_server = app.world().resource::<AssetServer>().clone();
        let (blocks, _) = load_block_definitions(&asset_server).expect("the block corpus loads");
        let items = load_item_definitions(&asset_server, &blocks).expect("the item corpus loads");
        (Blocks(Arc::new(blocks)), Items(Arc::new(items)))
    })
}

pub(crate) fn world() -> (World, Entity, Entity) {
    let (blocks, items) = corpus();
    let mut registry = RegistryAccess::default();
    registry.register(Box::new(RegistrySnapshotErased::from_entries(
        "minecraft:item",
        vec![(ResourceLocation::minecraft("stone"), None)],
        None,
    )));
    let mut world = World::new();
    world.insert_resource(blocks.clone());
    world.insert_resource(items.clone());
    world.insert_resource(registry);
    world.init_resource::<DirtyStacks>();
    world.init_resource::<MenuResync>();
    world.init_resource::<Messages<OutboundPlayerPacket>>();
    let anchor = world.spawn_empty().id();
    let player = world
        .spawn((
            Player,
            SlotTable::fixed(slots::COUNT),
            SelectedHotbarSlot(3),
            HostAnchor(anchor),
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

pub(crate) fn stone(world: &mut World) -> Entity {
    let value = ItemStackValue {
        item: ResourceKey::from_location(ResourceLocation::minecraft("stone")),
        count: Bounded(7),
        components: ComponentPatch::EMPTY,
    };
    mutate::spawn_stack(world, &value, &corpus().1).unwrap()
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
fn dirty_cells_become_set_slot_and_cursor_packets() {
    let (mut world, player, _anchor) = world();
    open_menus(&mut world);
    sync_stack_slots(&mut world);
    drain(&mut world);

    let stack = stone(&mut world);
    mutate::move_stack(&mut world, stack, player, slots::HOTBAR.start).unwrap();
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

    mutate::move_stack(&mut world, stack, player, slots::CARRIED).unwrap();
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

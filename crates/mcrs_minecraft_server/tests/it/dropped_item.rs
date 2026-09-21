use crate::inventory_sync::{corpus, drain, place, set_count, stone, world};
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::system::RunSystemOnce;
use bevy_ecs::world::World;
use bevy_math::DVec3;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::codec::Bounded;
use mcrs_minecraft_core::{BlockPos, ResourceKey, ResourceLocation, SectionPos};
use mcrs_minecraft_inventory::{CurrentMenu, Menu};
use mcrs_minecraft_inventory::{MenuContainer, Op, Slot};
use mcrs_minecraft_item::{DroppedItem, ItemStack, SlotTable, WireStack, slots};
use mcrs_minecraft_level::entity::mob::EntityKind;
use mcrs_minecraft_level::entity::physics::{Transform, Velocity};
use mcrs_minecraft_level::palette::ChunkBlocks;
use mcrs_minecraft_level::world::dimension::InDimension;
use mcrs_minecraft_level::world::lifecycle::level::SectionLevels;
use mcrs_minecraft_level::world::lifecycle::ticket::{
    SectionTickets, Ticket, propagate_section_levels,
};
use mcrs_minecraft_level::world::storage::block_entity::BlockEntityPos;
use mcrs_minecraft_level::world::storage::section::SectionIndex;
use mcrs_minecraft_protocol::item::{ComponentPatch, ItemStackValue, ItemStackWithSlot, RawStack};
use mcrs_minecraft_server::world::block_entity::{BlockEntity, spawn_block_entities};
use mcrs_minecraft_server::world::bus::PacketPayload;
use mcrs_minecraft_server::world::entity::item::pickup::pickup_items;
use mcrs_minecraft_server::world::entity::item::tick::tick_dropped_items;
use mcrs_minecraft_server::world::entity::item::{BlockDrop, spawn_dropped};
use mcrs_minecraft_server::world::item::chest::{
    OpenContainerRequest, close_dead_menus, open_containers,
};
use mcrs_minecraft_server::world::item::click::{CloseContainerRequest, close_menus, commit};
use mcrs_minecraft_server::world::item::menu::open_menus;
use mcrs_minecraft_server::world::item::sync::sync_stack_slots;
use mcrs_minecraft_worldgen_feature_place::block_entity::{ContainerData, GeneratedBlockEntity};

const FLOOR_TOP: f64 = 1.0;

/// A dimension whose only section is ticking and has a stone floor at y = 0.
fn dimension(world: &mut World) -> Entity {
    let (blocks, _) = corpus();
    let stone = VoxelId(blocks.default_state("minecraft:stone").0);
    let mut tickets = SectionTickets::default();
    tickets.add(SectionPos::new(0, 0, 0), Ticket::player_simulation(1));
    let dim = world
        .spawn((tickets, SectionLevels::default(), SectionIndex::default()))
        .id();
    let mut palette = ChunkBlocks::default();
    for x in 0..16 {
        for z in 0..16 {
            palette.make_mut().set_cell(x, 0, z, stone);
        }
    }
    let section = world
        .spawn((SectionPos::new(0, 0, 0), InDimension(dim), palette))
        .id();
    world
        .get_mut::<SectionIndex>(dim)
        .unwrap()
        .insert(SectionPos::new(0, 0, 0), section);
    world.run_system_once(propagate_section_levels).unwrap();
    dim
}

fn standing(world: &mut World, player: Entity, dim: Entity) {
    world
        .entity_mut(player)
        .insert((InDimension(dim), Transform::from_xyz(8.5, FLOOR_TOP, 8.5)));
}

fn stone_value(count: u8) -> ItemStackValue {
    ItemStackValue {
        item: ResourceKey::from_location(ResourceLocation::minecraft("stone")),
        count: Bounded(i32::from(count)),
        components: ComponentPatch::EMPTY,
    }
}

fn resting_item(world: &mut World, dim: Entity, count: u8) -> Entity {
    spawn_dropped(
        world,
        stone_value(count),
        dim,
        DVec3::new(8.5, FLOOR_TOP, 8.5),
        DVec3::ZERO,
        0,
        None,
    )
}

fn tick(world: &mut World, times: usize) {
    for _ in 0..times {
        tick_dropped_items(world);
    }
}

#[test]
fn a_thrown_stack_becomes_an_item_entity_in_front_of_the_player() {
    let (mut world, player, _) = world();
    let dim = dimension(&mut world);
    standing(&mut world, player, dim);
    let stack = stone(&mut world);
    place(&mut world, stack, player, slots::HOTBAR.start);
    commit(
        &mut world,
        vec![Op::Drop {
            from: Slot::new(player, slots::HOTBAR.start),
            count: 7,
            thrower: player,
        }],
    );

    let entity = world.entity(stack);
    assert_eq!(entity.get::<DroppedItem>().unwrap().pickup_delay, 40);
    assert_eq!(entity.get::<EntityKind>().unwrap().0.protocol_id, 72);
    let at = entity.get::<Transform>().unwrap().translation;
    assert_eq!(at, DVec3::new(8.5, FLOOR_TOP + 1.62 - 0.3, 8.5));
    let velocity = entity.get::<Velocity>().unwrap().0;
    assert!(velocity.z > 0.25 && velocity.z < 0.35, "{velocity:?}");
    assert!(velocity.y > 0.0 && velocity.y < 0.2, "{velocity:?}");

    sync_stack_slots(&mut world);
    assert!(world.get::<WireStack>(stack).is_some());
}

#[test]
fn an_item_falls_onto_the_floor_and_ages() {
    let (mut world, _, _) = world();
    let dim = dimension(&mut world);
    let stack = spawn_dropped(
        &mut world,
        stone_value(7),
        dim,
        DVec3::new(8.5, 4.0, 8.5),
        DVec3::ZERO,
        40,
        None,
    );
    tick(&mut world, 100);
    let entity = world.entity(stack);
    assert_eq!(entity.get::<Transform>().unwrap().translation.y, FLOOR_TOP);
    let item = entity.get::<DroppedItem>().unwrap();
    assert_eq!(item.pickup_delay, 0);
    assert_eq!(item.age, 100);
    tick(&mut world, 5900);
    assert!(
        world.get_entity(stack).is_err(),
        "the item despawns at 6000"
    );
}

#[test]
fn resting_items_merge_into_the_larger_stack_every_forty_ticks() {
    let (mut world, _, _) = world();
    let dim = dimension(&mut world);
    let small = resting_item(&mut world, dim, 3);
    let large = resting_item(&mut world, dim, 7);
    tick(&mut world, 39);
    assert_eq!(world.get::<ItemStack>(small).unwrap().count(), 3);
    tick(&mut world, 1);
    assert!(world.get_entity(small).is_err());
    assert_eq!(world.get::<ItemStack>(large).unwrap().count(), 10);
}

#[test]
fn pickup_fills_the_held_slot_first_and_announces_the_take_before_the_stack_moves() {
    let (mut world, player, _) = world();
    let dim = dimension(&mut world);
    standing(&mut world, player, dim);
    for index in slots::HOTBAR.chain(slots::MAIN) {
        let filler = stone(&mut world);
        let count = if index == slots::held(3) { 60 } else { 64 };
        set_count(&mut world, filler, count);
        place(&mut world, filler, player, index);
    }
    let item = resting_item(&mut world, dim, 7);
    drain(&mut world);

    pickup_items(&mut world);

    let packets = drain(&mut world);
    assert!(
        matches!(
            packets.first().map(|packet| &packet.data),
            Some(PacketPayload::TakeItemEntity { amount: 7, player_id, .. })
                if *player_id == player.index_u32() as i32
        ),
        "{packets:?}"
    );
    let held = world
        .get::<SlotTable>(player)
        .unwrap()
        .get(slots::held(3))
        .unwrap();
    assert_eq!(world.get::<ItemStack>(held).unwrap().count(), 64);
    assert_eq!(world.get::<ItemStack>(item).unwrap().count(), 3);

    let free = world
        .get::<SlotTable>(player)
        .unwrap()
        .get(slots::MAIN.start)
        .unwrap();
    set_count(&mut world, free, 0);
    pickup_items(&mut world);
    assert!(world.get_entity(item).is_err());
    let main = world
        .get::<SlotTable>(player)
        .unwrap()
        .get(slots::MAIN.start)
        .unwrap();
    assert_eq!(world.get::<ItemStack>(main).unwrap().count(), 3);
}

#[test]
fn a_block_drop_scatters_inside_the_broken_block() {
    let (mut world, _, _) = world();
    let dim = dimension(&mut world);
    world.init_resource::<Messages<BlockDrop>>();
    world.write_message(BlockDrop {
        dim,
        pos: BlockPos::new(3, 1, 3),
        item: ResourceLocation::minecraft("stone"),
        count: 2,
    });
    world
        .run_system_once(mcrs_minecraft_server::world::entity::item::spawn_block_drops)
        .unwrap();
    let mut dropped = world.query::<(&Transform, &DroppedItem, &ItemStack)>();
    let (transform, item, stack) = dropped.single(&world).unwrap();
    let at = transform.translation;
    assert!(
        (at.x - 3.5).abs() <= 0.25 && (at.z - 3.5).abs() <= 0.25,
        "{at:?}"
    );
    assert!((at.y - 1.375).abs() <= 0.25, "{at:?}");
    assert_eq!(item.pickup_delay, 10);
    assert_eq!(stack.count(), 2);
}

fn chest(world: &mut World, dim: Entity) -> Entity {
    world.init_resource::<Messages<OpenContainerRequest>>();
    let chest = world
        .spawn((
            BlockEntityPos(BlockPos::new(1, 1, 1)),
            InDimension(dim),
            SlotTable::fixed(27),
        ))
        .id();
    let stack = stone(world);
    place(world, stack, chest, 3);
    chest
}

#[test]
fn opening_a_chest_swaps_the_menu_and_sends_its_contents() {
    let (mut world, player, _) = world();
    let dim = dimension(&mut world);
    let chest = chest(&mut world, dim);
    open_menus(&mut world);
    sync_stack_slots(&mut world);
    let inventory_menu = world.get::<CurrentMenu>(player).unwrap().0;
    drain(&mut world);

    world.write_message(OpenContainerRequest {
        player,
        container: chest,
    });
    open_containers(&mut world);
    sync_stack_slots(&mut world);

    let menu = world.get::<CurrentMenu>(player).unwrap().0;
    assert_ne!(menu, inventory_menu);
    assert_eq!(world.get::<Menu>(menu).unwrap().container_id, 1);
    assert_eq!(world.get::<MenuContainer>(menu).unwrap().0, chest);
    let packets = drain(&mut world);
    assert!(matches!(
        packets[0].data,
        PacketPayload::OpenScreen {
            container_id: 1,
            menu_type: 2,
            ..
        }
    ));
    let PacketPayload::ContainerSetContent {
        container_id: 1,
        slots,
        ..
    } = &packets[1].data
    else {
        panic!("{packets:?}");
    };
    assert_eq!(slots.len(), 63);
    assert_ne!(slots[3], RawStack::EMPTY);
    assert_eq!(slots[4], RawStack::EMPTY);

    world.init_resource::<Messages<CloseContainerRequest>>();
    world.write_message(CloseContainerRequest {
        player,
        container_id: 1,
    });
    close_menus(&mut world);
    assert_eq!(world.get::<CurrentMenu>(player).unwrap().0, inventory_menu);
    assert!(world.get_entity(menu).is_err());

    world.write_message(OpenContainerRequest {
        player,
        container: chest,
    });
    open_containers(&mut world);
    drain(&mut world);
    let menu = world.get::<CurrentMenu>(player).unwrap().0;
    assert_eq!(world.get::<Menu>(menu).unwrap().container_id, 2);
    world.despawn(chest);
    close_dead_menus(&mut world);
    assert_eq!(world.get::<CurrentMenu>(player).unwrap().0, inventory_menu);
    assert!(world.get_entity(menu).is_err());
    let packets = drain(&mut world);
    assert!(
        matches!(packets[0].data, PacketPayload::ContainerClose(2)),
        "{packets:?}"
    );
}

#[test]
fn a_loaded_chest_holds_its_saved_items_as_stacks() {
    let (mut world, _, _) = world();
    let dim = dimension(&mut world);
    let entry = |slot: u8| ItemStackWithSlot {
        slot,
        stack: ItemStackValue {
            item: ResourceKey::from_location(ResourceLocation::minecraft("stone")),
            count: Bounded(5),
            components: ComponentPatch::EMPTY,
        },
    };
    let data = ContainerData {
        x: 2,
        y: 1,
        z: 2,
        loot_table: None,
        loot_table_seed: 0,
        items: vec![entry(4), entry(27)],
        custom_name: None,
        components: None,
    };
    let mut queue = bevy_ecs::world::CommandQueue::default();
    let mut commands = bevy_ecs::system::Commands::new(&mut queue, &world);
    spawn_block_entities(
        &mut commands,
        InDimension(dim),
        vec![GeneratedBlockEntity::Chest(data)],
    );
    queue.apply(&mut world);

    let mut chests = world.query::<(&BlockEntity, &SlotTable)>();
    let (block_entity, table) = chests.single(&world).unwrap();
    let GeneratedBlockEntity::Chest(data) = &block_entity.0 else {
        panic!()
    };
    assert!(data.items.is_empty());
    assert_eq!(table.len(), 27);
    assert_eq!(table.iter().count(), 1);
    let stack = table.get(4).unwrap();
    assert_eq!(world.get::<ItemStack>(stack).unwrap().count(), 5);
}

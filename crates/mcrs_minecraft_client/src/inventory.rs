use bevy::ecs::system::Command;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_inventory::{
    MenuLayout, Op, Slot, Transaction, container_menu_layout, menu_slots, stack_in,
};
use mcrs_minecraft_item::{Items, SelectedHotbarSlot, SlotTable, item_of, slots};
use mcrs_minecraft_network::ConnectionState;
use mcrs_minecraft_network::client::{ClientConnection, ClientNetworkSystems, ReceivedRegistries};
use mcrs_minecraft_network::event::ReceivedPacketEvent;
use mcrs_minecraft_protocol::item::{ItemStackValue, RawStack};
use mcrs_minecraft_protocol::packets::game::clientbound::{
    ClientboundContainerClose, ClientboundContainerSetContent, ClientboundContainerSetSlot,
    ClientboundOpenScreen, ClientboundSetCursorItem, ClientboundSetHeldSlot,
    ClientboundSetPlayerInventory,
};
use mcrs_minecraft_protocol::packets::game::serverbound::{
    ServerboundContainerClose, ServerboundSetCarriedItem,
};
use mcrs_minecraft_protocol::{GameMode, VarInt, WritePacket};
use mcrs_minecraft_registry::{ChainLookup, RegistryLookup, StaticRegistryTable};

use crate::asset_corpus;
use crate::player::{self, Player};

pub const REGISTRY_REPORT: &str = "mcrs/reports/registries.json";

/// The server's state id for the holder's menu, echoed back on every click.
#[derive(Component, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContainerSeqno(pub u32);

#[derive(Component, Debug)]
pub struct OpenMenu {
    pub container_id: i32,
}

#[derive(Resource, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    #[default]
    None,
    Inventory,
    Container(Entity),
    GameModeSwitcher(GameMode),
}

pub struct InventoryPlugin;

impl Plugin for InventoryPlugin {
    fn build(&self, app: &mut App) {
        let report = asset_corpus().join(REGISTRY_REPORT);
        let table = StaticRegistryTable::load(&report)
            .unwrap_or_else(|err| panic!("{}: {err}", report.display()));
        app.insert_resource(table)
            .init_resource::<Screen>()
            .add_observer(receive_inventory_packets)
            .add_systems(
                Update,
                (select_hotbar_slot, toggle_inventory)
                    .after(ClientNetworkSystems::Receive)
                    .before(ClientNetworkSystems::Flush)
                    .after(player::grab_cursor_on_click),
            );
    }

    fn finish(&self, app: &mut App) {
        let world = app.world();
        let table = world.resource::<StaticRegistryTable>();
        for entry in world.resource::<Items>().iter() {
            let reported = table.id("item", &entry.identifier);
            assert_eq!(
                reported,
                Some(u32::from(entry.id.0)),
                "{} is #{} in the item corpus but {reported:?} in {REGISTRY_REPORT}",
                entry.identifier,
                entry.id.0
            );
        }
    }
}

pub fn inventory_index_to_cell(index: i32) -> Option<u16> {
    u8::try_from(index)
        .ok()
        .and_then(slots::from_inventory_index)
}

fn resolve(raw: &RawStack, lookup: &dyn RegistryLookup) -> anyhow::Result<Option<ItemStackValue>> {
    let slot = raw.resolve(lookup)?;
    if slot.is_empty() {
        return Ok(None);
    }
    slot.to_value(lookup).map(Some)
}

fn receive_inventory_packets(
    event: On<ReceivedPacketEvent>,
    connections: Query<(&ConnectionState, &ReceivedRegistries)>,
    table: Res<StaticRegistryTable>,
    mut selected: Query<&mut SelectedHotbarSlot, With<Player>>,
    mut commands: Commands,
) {
    let Ok((ConnectionState::Game, received)) = connections.get(event.entity) else {
        return;
    };
    let lookup = ChainLookup(&[&*table as &dyn RegistryLookup, received]);
    if let Some(packet) = event.decode::<ClientboundContainerSetContent>() {
        let slots: anyhow::Result<Vec<_>> = packet
            .slot_data
            .iter()
            .map(|raw| resolve(raw, &lookup))
            .collect();
        let (Ok(slots), Ok(carried)) = (slots, resolve(&packet.carried_item, &lookup)) else {
            warn!(
                "container_set_content {}: a stack failed to decode",
                packet.container_id.0
            );
            return;
        };
        commands.queue(move |world: &mut World| {
            apply_set_content(
                world,
                packet.container_id.0,
                packet.state_seqno.0,
                slots,
                carried,
            );
        });
    } else if let Some(packet) = event.decode::<ClientboundContainerSetSlot>() {
        let Ok(item) = resolve(&packet.item, &lookup) else {
            warn!(
                "container_set_slot {}/{}: the stack failed to decode",
                packet.container_id.0, packet.slot
            );
            return;
        };
        commands.queue(move |world: &mut World| {
            apply_set_slot(
                world,
                packet.container_id.0,
                packet.state_seqno.0,
                packet.slot,
                item,
            );
        });
    } else if let Some(packet) = event.decode::<ClientboundSetCursorItem>() {
        let Ok(item) = resolve(&packet.contents, &lookup) else {
            warn!("set_cursor_item: the stack failed to decode");
            return;
        };
        commands.queue(move |world: &mut World| {
            if let Some(player) = player(world) {
                set_cell(world, Slot::new(player, slots::CARRIED), item);
            }
        });
    } else if let Some(packet) = event.decode::<ClientboundSetPlayerInventory>() {
        let Ok(item) = resolve(&packet.contents, &lookup) else {
            warn!(
                "set_player_inventory {}: the stack failed to decode",
                packet.slot.0
            );
            return;
        };
        // ponytail: body and saddle (41, 42) are dropped until the player holds those cells.
        let Some(cell) = inventory_index_to_cell(packet.slot.0) else {
            warn!("set_player_inventory {}: no such cell", packet.slot.0);
            return;
        };
        commands.queue(move |world: &mut World| {
            if let Some(player) = player(world) {
                set_cell(world, Slot::new(player, cell), item);
            }
        });
    } else if let Some(packet) = event.decode::<ClientboundSetHeldSlot>() {
        if let Ok(slot) = u8::try_from(packet.slot.0)
            && usize::from(slot) < slots::HOTBAR.len()
        {
            for mut selected in &mut selected {
                selected.set_if_neq(SelectedHotbarSlot(slot));
            }
        }
    } else if let Some(packet) = event.decode::<ClientboundOpenScreen>() {
        let Some(menu_type) = table.name("menu", packet.menu_type.0 as u32).cloned() else {
            warn!("open_screen: unknown menu type {}", packet.menu_type.0);
            return;
        };
        commands.queue(move |world: &mut World| {
            open_screen(world, packet.container_id.0, menu_type);
        });
    } else if event.decode::<ClientboundContainerClose>().is_some() {
        commands.queue(close_screen);
    }
}

fn player(world: &mut World) -> Option<Entity> {
    world
        .query_filtered::<Entity, With<Player>>()
        .iter(world)
        .next()
}

fn open_menu(world: &World) -> Option<(Entity, i32)> {
    let Screen::Container(menu) = *world.resource::<Screen>() else {
        return None;
    };
    Some((menu, world.get::<OpenMenu>(menu)?.container_id))
}

/// The holder a container id addresses: the player for 0, the open menu when
/// the ids agree, nothing for a menu that is no longer open.
fn container(world: &mut World, container_id: i32) -> Option<Entity> {
    if container_id == 0 {
        return player(world);
    }
    open_menu(world)
        .filter(|(_, id)| *id == container_id)
        .map(|(menu, _)| menu)
}

fn holder_cell(world: &World, container: Entity, index: usize) -> Option<Slot> {
    match world.get::<MenuLayout>(container) {
        Some(layout) => layout.0.get(index).copied(),
        None => u16::try_from(index)
            .ok()
            .filter(|cell| usize::from(*cell) < slots::MENU_COUNT)
            .map(|cell| Slot::new(container, cell)),
    }
}

/// A cell keeps its stack entity when the item stays the same, so a count or
/// component change is a revision rather than a respawn.
fn set_cell(world: &mut World, cell: Slot, value: Option<ItemStackValue>) {
    let current = stack_in(world, cell);
    let same_item = |world: &World, stack: Entity, value: &ItemStackValue| {
        item_of(world, stack) == world.resource::<Items>().id_of(value.item.as_str())
    };
    let ops = match (current, value) {
        (None, None) => Vec::new(),
        (Some(stack), None) => vec![Op::Despawn { stack }],
        (Some(stack), Some(value)) if same_item(world, stack, &value) => {
            vec![Op::Apply { stack, value }]
        }
        (current, Some(value)) => current
            .map(|stack| Op::Despawn { stack })
            .into_iter()
            .chain(std::iter::once(Op::Spawn { value, to: cell }))
            .collect(),
    };
    Transaction(ops).apply(world);
}

fn set_seqno(world: &mut World, container: Entity, seqno: i32) {
    if let Some(mut current) = world.get_mut::<ContainerSeqno>(container) {
        current.set_if_neq(ContainerSeqno(seqno as u32));
    }
}

fn apply_set_content(
    world: &mut World,
    container_id: i32,
    seqno: i32,
    stacks: Vec<Option<ItemStackValue>>,
    carried: Option<ItemStackValue>,
) {
    let Some(container) = container(world, container_id) else {
        return;
    };
    for (index, value) in stacks.into_iter().enumerate() {
        match holder_cell(world, container, index) {
            Some(cell) => set_cell(world, cell, value),
            None if value.is_some() => {
                warn!("container_set_content {container_id}: slot {index} has nowhere to land");
            }
            None => {}
        }
    }
    if let Some(player) = player(world) {
        set_cell(world, Slot::new(player, slots::CARRIED), carried);
    }
    set_seqno(world, container, seqno);
}

fn apply_set_slot(
    world: &mut World,
    container_id: i32,
    seqno: i32,
    slot: i16,
    item: Option<ItemStackValue>,
) {
    let Some(container) = container(world, container_id) else {
        return;
    };
    let Some(cell) = usize::try_from(slot)
        .ok()
        .and_then(|index| holder_cell(world, container, index))
    else {
        return;
    };
    set_cell(world, cell, item);
    set_seqno(world, container, seqno);
}

fn open_screen(world: &mut World, container_id: i32, menu_type: ResourceLocation) {
    let Some(player) = player(world) else {
        return;
    };
    let Some(slots) = menu_slots(menu_type.as_str()) else {
        warn!("open_screen: {menu_type} has no slot layout, the screen stays closed");
        return;
    };
    if let Some((menu, _)) = open_menu(world) {
        world.despawn(menu);
    }
    let menu = world.spawn_empty().id();
    world.entity_mut(menu).insert((
        OpenMenu { container_id },
        SlotTable::fixed(usize::from(slots.own) + usize::from(slots.trailing_result)),
        ContainerSeqno::default(),
        MenuLayout(container_menu_layout(menu, player, slots)),
    ));
    *world.resource_mut::<Screen>() = Screen::Container(menu);
    set_cursor_grabbed(world, false);
}

fn close_screen(world: &mut World) {
    if let Some((menu, _)) = open_menu(world) {
        world.despawn(menu);
    }
    *world.resource_mut::<Screen>() = Screen::None;
    set_cursor_grabbed(world, true);
}

fn set_cursor_grabbed(world: &mut World, grabbed: bool) {
    for mut cursor in world
        .query_filtered::<&mut CursorOptions, With<PrimaryWindow>>()
        .iter_mut(world)
    {
        grab(&mut cursor, grabbed);
    }
}

pub(crate) fn grab(cursor: &mut CursorOptions, grabbed: bool) {
    cursor.grab_mode = if grabbed {
        CursorGrabMode::Locked
    } else {
        CursorGrabMode::None
    };
    cursor.visible = !grabbed;
}

const HOTBAR_KEYS: [KeyCode; 9] = [
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
    KeyCode::Digit6,
    KeyCode::Digit7,
    KeyCode::Digit8,
    KeyCode::Digit9,
];

fn select_hotbar_slot(
    keys: Res<ButtonInput<KeyCode>>,
    screen: Res<Screen>,
    cursor: Single<&CursorOptions, With<PrimaryWindow>>,
    mut selected: Single<&mut SelectedHotbarSlot, With<Player>>,
    connection: Option<Single<&mut ClientConnection>>,
) {
    if *screen != Screen::None || cursor.grab_mode == CursorGrabMode::None {
        return;
    }
    let Some(slot) = HOTBAR_KEYS.iter().position(|key| keys.just_pressed(*key)) else {
        return;
    };
    if selected.set_if_neq(SelectedHotbarSlot(slot as u8))
        && let Some(mut connection) = connection
    {
        connection.write_packet(&ServerboundSetCarriedItem { slot: slot as u16 });
    }
}

fn toggle_inventory(
    keys: Res<ButtonInput<KeyCode>>,
    mut screen: ResMut<Screen>,
    mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>,
    mut window: Single<&mut Window, With<PrimaryWindow>>,
    connection: Option<Single<&mut ClientConnection>>,
    menus: Query<&OpenMenu>,
    mut commands: Commands,
) {
    let escape = keys.just_pressed(KeyCode::Escape);
    let toggle = keys.just_pressed(KeyCode::KeyE);
    let closing = match *screen {
        Screen::None => {
            if toggle && cursor.grab_mode != CursorGrabMode::None {
                *screen = Screen::Inventory;
                grab(&mut cursor, false);
                let center = Vec2::new(
                    window.physical_width() as f32,
                    window.physical_height() as f32,
                ) / 2.0;
                window.set_physical_cursor_position(Some(center.as_dvec2()));
            }
            return;
        }
        Screen::Inventory if escape || toggle => 0,
        Screen::Container(menu) if escape || toggle => {
            let id = menus.get(menu).map_or(0, |menu| menu.container_id);
            commands.entity(menu).despawn();
            id
        }
        _ => return,
    };
    if let Some(mut connection) = connection {
        connection.write_packet(&ServerboundContainerClose {
            container_id: VarInt(closing),
        });
    }
    *screen = Screen::None;
    grab(&mut cursor, true);
}

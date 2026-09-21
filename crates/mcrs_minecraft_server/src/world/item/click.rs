use crate::world::entity::player::ability::PlayerGameMode;
use crate::world::entity::player::player_action::{PlayerAction, PlayerActionKind};
use crate::world::inventory::held_stack;
use crate::world::item::chest::{MenuContainer, close_container_menu};
use crate::world::item::menu::{CurrentMenu, Menu, MenuLayout};
use crate::world::item::sync::MenuResync;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::{Message, MessageCursor, Messages};
use bevy_ecs::prelude::{Local, MessageWriter, On};
use bevy_ecs::world::World;
use mcrs_minecraft_assets::access::RegistryAccess;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_item::mutate::{self, MoveError};
use mcrs_minecraft_item::{
    DirtyStacks, ItemStack, Items, SelectedHotbarSlot, SlotTable, effective, is_stackable,
    max_stack_size, same_item_same_components, slots, stack_to_slot, stack_to_value,
};
use mcrs_minecraft_level::entity::player::Player;
use mcrs_minecraft_network::event::ReceivedPacketEvent;
use mcrs_minecraft_protocol::GameMode;
use mcrs_minecraft_protocol::entity::EquipmentSlot;
use mcrs_minecraft_protocol::item::component::Equippable;
use mcrs_minecraft_protocol::item::{ContainerInput, HashedSlot, RawDelimitedStack, Slot};
use mcrs_minecraft_protocol::packets::game::serverbound::{
    ServerboundContainerClick, ServerboundContainerClose, ServerboundSetCreativeModeSlot,
};
use mcrs_minecraft_registry::{ChainLookup, RegistryLookup};

const SLOT_CLICKED_OUTSIDE: i16 = -999;
const SWAP_OFFHAND_BUTTON: u8 = 40;
/// The main inventory and hotbar cells every container menu ends with.
pub const PLAYER_MENU_CELLS: usize = (slots::HOTBAR.end - slots::MAIN.start) as usize;

#[derive(Message, Debug)]
pub struct ContainerClickRequest {
    pub player: Entity,
    pub container_id: i32,
    pub state_id: i32,
    pub slot: i16,
    pub button: u8,
    pub input: ContainerInput,
    pub changed: Vec<(u16, Option<HashedSlot>)>,
    pub carried: Option<HashedSlot>,
}

#[derive(Message, Debug)]
pub struct CreativeSlotRequest {
    pub player: Entity,
    pub slot: i16,
    pub item: RawDelimitedStack,
}

#[derive(Message, Debug)]
pub struct CloseContainerRequest {
    pub player: Entity,
    pub container_id: i32,
}

pub fn decode_container_click(
    event: On<ReceivedPacketEvent>,
    mut out: MessageWriter<ContainerClickRequest>,
) {
    let Some(pkt) = event.decode::<ServerboundContainerClick>() else {
        return;
    };
    out.write(ContainerClickRequest {
        player: event.entity,
        container_id: pkt.container_id.0,
        state_id: pkt.state_seqno.0,
        slot: pkt.slot_index,
        button: pkt.button,
        input: pkt.container_input,
        changed: pkt.changed_slots.0,
        carried: pkt.carried_item,
    });
}

pub fn decode_creative_slot(
    event: On<ReceivedPacketEvent>,
    mut out: MessageWriter<CreativeSlotRequest>,
) {
    let Some(pkt) = event.decode::<ServerboundSetCreativeModeSlot>() else {
        return;
    };
    out.write(CreativeSlotRequest {
        player: event.entity,
        slot: pkt.slot,
        item: pkt.item,
    });
}

pub fn decode_container_close(
    event: On<ReceivedPacketEvent>,
    mut out: MessageWriter<CloseContainerRequest>,
) {
    let Some(pkt) = event.decode::<ServerboundContainerClose>() else {
        return;
    };
    out.write(CloseContainerRequest {
        player: event.entity,
        container_id: pkt.container_id.0,
    });
}

pub fn handle_container_clicks(world: &mut World) {
    let requests: Vec<ContainerClickRequest> = world
        .resource_mut::<Messages<ContainerClickRequest>>()
        .drain()
        .collect();
    if requests.is_empty() {
        return;
    }
    let items = world.resource::<Items>().clone();
    for req in requests {
        let Some(menu) = world
            .get::<CurrentMenu>(req.player)
            .map(|current| current.0)
        else {
            continue;
        };
        let Some((container_id, state_id)) = world
            .get::<Menu>(menu)
            .map(|menu| (menu.container_id, menu.state_id))
        else {
            continue;
        };
        if i32::from(container_id) != req.container_id {
            continue;
        }
        if world
            .get::<PlayerGameMode>(req.player)
            .is_some_and(|mode| mode.0 == GameMode::Spectator)
        {
            world.resource_mut::<MenuResync>().menus_full.push(menu);
            continue;
        }
        let layout = world.get::<MenuLayout>(menu).unwrap().0.clone();
        let valid_slot = req.slot == -1
            || req.slot == SLOT_CLICKED_OUTSIDE
            || usize::try_from(req.slot).is_ok_and(|slot| slot < layout.len());
        if !valid_slot {
            tracing::debug!(player = ?req.player, slot = req.slot, "click on an invalid slot");
            continue;
        }
        // ponytail: drag and double-click are not applied; the full resend
        // rolls the client's prediction back. Upgrade: the drag header state
        // machine and the two-pass gather over the layout.
        let mut full = req.state_id != i32::from(state_id)
            || matches!(
                req.input,
                ContainerInput::QuickCraft | ContainerInput::PickupAll
            );
        if let Err(error) = apply(world, req.player, &layout, &req, &items) {
            tracing::debug!(%error, player = ?req.player, "a click could not be applied");
            full = true;
        }
        let mut disagreeing = Vec::new();
        for (slot, hashed) in &req.changed {
            match layout.get(usize::from(*slot)) {
                Some(cell) if !client_matches(world, *cell, hashed.as_ref(), &items) => {
                    disagreeing.push(*cell)
                }
                Some(_) => {}
                None => {
                    tracing::debug!(player = ?req.player, slot, "changed slot outside the menu")
                }
            }
        }
        let carried = (req.player, slots::CARRIED);
        if !client_matches(world, carried, req.carried.as_ref(), &items) {
            disagreeing.push(carried);
        }
        world
            .resource_mut::<DirtyStacks>()
            .cells
            .extend(disagreeing);
        if full {
            world.resource_mut::<MenuResync>().menus_full.push(menu);
        }
    }
}

fn client_matches(
    world: &World,
    cell: (Entity, u16),
    hashed: Option<&HashedSlot>,
    items: &Items,
) -> bool {
    let server =
        stack_in(world, cell).map_or(Slot::EMPTY, |stack| stack_to_slot(world, stack, items));
    hashed.map_or(server.is_empty(), |hashed| hashed.matches(&server))
}

fn stack_in(world: &World, (holder, index): (Entity, u16)) -> Option<Entity> {
    world.get::<SlotTable>(holder)?.get(index)
}

fn count(world: &World, stack: Entity) -> u8 {
    world
        .get::<ItemStack>(stack)
        .map_or(0, |stack| stack.count())
}

fn armour_cell(world: &World, stack: Entity, items: &Items) -> Option<u16> {
    match effective::<Equippable>(world.entity(stack), items)?.slot {
        EquipmentSlot::Head => Some(slots::ARMOR_HEAD),
        EquipmentSlot::Chest => Some(slots::ARMOR_CHEST),
        EquipmentSlot::Legs => Some(slots::ARMOR_LEGS),
        EquipmentSlot::Feet => Some(slots::ARMOR_FEET),
        _ => None,
    }
}

fn is_offhand_item(world: &World, stack: Entity, items: &Items) -> bool {
    effective::<Equippable>(world.entity(stack), items)
        .is_some_and(|equippable| equippable.slot == EquipmentSlot::OffHand)
}

/// How many of `stack` the cell may hold, `None` when it may not hold it at all.
/// ponytail: the only cell rules are the player's result and armour cells;
/// a container with its own limit (a chest's 64) caps here when it exists.
fn cell_max(
    world: &World,
    (holder, index): (Entity, u16),
    stack: Entity,
    items: &Items,
) -> Option<u8> {
    let max = max_stack_size(world.entity(stack), items);
    if world.get::<Player>(holder).is_none() {
        return Some(max);
    }
    match index {
        slots::RESULT => None,
        slots::ARMOR_HEAD..=slots::ARMOR_FEET => {
            (armour_cell(world, stack, items) == Some(index)).then_some(1)
        }
        _ => Some(max),
    }
}

fn place(world: &mut World, stack: Entity, cell: (Entity, u16)) -> Result<(), MoveError> {
    mutate::move_stack(world, stack, cell.0, cell.1)
}

/// The stack itself when all of it is taken, else the split-off part.
fn take(world: &mut World, stack: Entity, amount: u8, items: &Items) -> Option<Entity> {
    if amount >= count(world, stack) {
        Some(stack)
    } else {
        mutate::split(world, stack, amount, items)
    }
}

/// Inserts up to `amount` of `stack` into the cell, which is empty or holds the
/// same item; whatever does not fit stays where it was.
fn safe_insert(
    world: &mut World,
    stack: Entity,
    cell: (Entity, u16),
    amount: u8,
    items: &Items,
) -> Result<(), MoveError> {
    let Some(max) = cell_max(world, cell, stack, items) else {
        return Ok(());
    };
    match stack_in(world, cell) {
        None => {
            let moving = amount.min(count(world, stack)).min(max);
            if moving == 0 {
                return Ok(());
            }
            let Some(moving) = take(world, stack, moving, items) else {
                return Ok(());
            };
            place(world, moving, cell)
        }
        Some(target) => {
            let room = max.saturating_sub(count(world, target)).min(amount);
            mutate::merge_into(
                world,
                stack,
                target,
                count(world, target).saturating_add(room),
            );
            Ok(())
        }
    }
}

/// Merges into same-item cells, then fills the first empty one; `cells` come
/// in the order they are tried. Returns whether anything moved.
fn move_to(
    world: &mut World,
    stack: Entity,
    cells: &[(Entity, u16)],
    items: &Items,
) -> Result<bool, MoveError> {
    let merged = merge_same(world, stack, cells, items)?;
    if world.get::<ItemStack>(stack).is_none() {
        return Ok(true);
    }
    Ok(fill_empty(world, stack, cells, items)? || merged)
}

fn merge_same(
    world: &mut World,
    stack: Entity,
    cells: &[(Entity, u16)],
    items: &Items,
) -> Result<bool, MoveError> {
    let mut moved = false;
    if !is_stackable(world.entity(stack), items) {
        return Ok(false);
    }
    for &cell in cells {
        let Some(target) = stack_in(world, cell) else {
            continue;
        };
        if !same_item_same_components(world, stack, target, items) {
            continue;
        }
        let Some(max) = cell_max(world, cell, stack, items) else {
            continue;
        };
        moved |= mutate::merge_into(world, stack, target, max) > 0;
        if world.get::<ItemStack>(stack).is_none() {
            return Ok(true);
        }
    }
    Ok(moved)
}

fn fill_empty(
    world: &mut World,
    stack: Entity,
    cells: &[(Entity, u16)],
    items: &Items,
) -> Result<bool, MoveError> {
    for &cell in cells {
        if stack_in(world, cell).is_some() {
            continue;
        }
        let Some(max) = cell_max(world, cell, stack, items) else {
            continue;
        };
        let Some(moving) = take(world, stack, max, items) else {
            continue;
        };
        place(world, moving, cell)?;
        return Ok(true);
    }
    Ok(false)
}

/// The cells a picked-up stack merges into, in vanilla's order: the held
/// slot, the offhand, then the hotbar and main inventory.
fn pickup_merge_cells(world: &World, player: Entity) -> Vec<(Entity, u16)> {
    let held = slots::held(
        world
            .get::<SelectedHotbarSlot>(player)
            .map_or(0, |selected| selected.0),
    );
    [held, slots::OFFHAND]
        .into_iter()
        .chain(
            slots::HOTBAR
                .chain(slots::MAIN)
                .filter(|index| *index != held),
        )
        .map(|index| (player, index))
        .collect()
}

fn pickup_empty_cells(player: Entity) -> Vec<(Entity, u16)> {
    slots::HOTBAR
        .chain(slots::MAIN)
        .map(|index| (player, index))
        .collect()
}

/// How many of `stack` the player's inventory can still take.
pub fn room_for(world: &World, player: Entity, stack: Entity, items: &Items) -> u32 {
    let max = max_stack_size(world.entity(stack), items);
    let mut room = 0u32;
    if is_stackable(world.entity(stack), items) {
        for cell in pickup_merge_cells(world, player) {
            if let Some(target) = stack_in(world, cell)
                && same_item_same_components(world, stack, target, items)
            {
                room += u32::from(max.saturating_sub(count(world, target)));
            }
        }
    }
    for cell in pickup_empty_cells(player) {
        if stack_in(world, cell).is_none() {
            room += u32::from(max);
        }
    }
    room
}

/// Stores an unheld stack the way a pickup does; what does not fit stays
/// in `stack`.
pub fn insert_stack(
    world: &mut World,
    player: Entity,
    stack: Entity,
    items: &Items,
) -> Result<bool, MoveError> {
    let merged = merge_same(world, stack, &pickup_merge_cells(world, player), items)?;
    if world.get::<ItemStack>(stack).is_none() {
        return Ok(true);
    }
    Ok(fill_empty(world, stack, &pickup_empty_cells(player), items)? || merged)
}

fn layout_range(
    layout: &[(Entity, u16)],
    range: std::ops::Range<usize>,
    backwards: bool,
) -> Vec<(Entity, u16)> {
    let cells = &layout[range];
    if backwards {
        cells.iter().rev().copied().collect()
    } else {
        cells.to_vec()
    }
}

fn quick_move(
    world: &mut World,
    player: Entity,
    layout: &[(Entity, u16)],
    slot: usize,
    items: &Items,
) -> Result<bool, MoveError> {
    let Some(stack) = stack_in(world, layout[slot]) else {
        return Ok(false);
    };
    if layout.len() != slots::MENU_COUNT {
        let container = layout.len() - PLAYER_MENU_CELLS;
        let cells = if slot < container {
            layout_range(layout, container..layout.len(), true)
        } else {
            layout_range(layout, 0..container, false)
        };
        return move_to(world, stack, &cells, items);
    }
    let slot = slot as u16;
    let main_and_hotbar = slots::MAIN.start as usize..slots::HOTBAR.end as usize;
    let cells = if slot == slots::RESULT {
        layout_range(layout, main_and_hotbar, true)
    } else if slot < slots::MAIN.start {
        layout_range(layout, main_and_hotbar, false)
    } else if let Some(armour) =
        armour_cell(world, stack, items).filter(|cell| stack_in(world, (player, *cell)).is_none())
    {
        vec![(player, armour)]
    } else if is_offhand_item(world, stack, items)
        && stack_in(world, (player, slots::OFFHAND)).is_none()
    {
        vec![(player, slots::OFFHAND)]
    } else if slots::MAIN.contains(&slot) {
        layout_range(
            layout,
            slots::HOTBAR.start as usize..slots::HOTBAR.end as usize,
            false,
        )
    } else if slots::HOTBAR.contains(&slot) {
        layout_range(
            layout,
            slots::MAIN.start as usize..slots::MAIN.end as usize,
            false,
        )
    } else {
        layout_range(layout, main_and_hotbar, false)
    };
    move_to(world, stack, &cells, items)
}

/// Puts a stack back into the player's hotbar and main inventory, dropping
/// what does not fit.
pub fn insert_or_drop(
    world: &mut World,
    player: Entity,
    stack: Entity,
    items: &Items,
) -> Result<(), MoveError> {
    let cells: Vec<(Entity, u16)> = slots::HOTBAR
        .chain(slots::MAIN)
        .map(|index| (player, index))
        .collect();
    move_to(world, stack, &cells, items)?;
    if world.get::<ItemStack>(stack).is_some() {
        drop_stack(world, player, stack);
    }
    Ok(())
}

pub fn drop_stack(world: &mut World, player: Entity, stack: Entity) {
    crate::world::entity::item::throw(world, player, stack);
}

fn apply(
    world: &mut World,
    player: Entity,
    layout: &[(Entity, u16)],
    req: &ContainerClickRequest,
    items: &Items,
) -> Result<(), MoveError> {
    let carried_cell = (player, slots::CARRIED);
    let carried = stack_in(world, carried_cell);
    let primary = req.button == 0;
    let clicked_cell = usize::try_from(req.slot).ok().map(|slot| layout[slot]);
    match req.input {
        ContainerInput::Pickup | ContainerInput::QuickMove if req.button > 1 => {}
        ContainerInput::Pickup | ContainerInput::QuickMove if req.slot == SLOT_CLICKED_OUTSIDE => {
            if let Some(carried) = carried {
                let amount = if primary { count(world, carried) } else { 1 };
                if let Some(thrown) = take(world, carried, amount, items) {
                    drop_stack(world, player, thrown);
                }
            }
        }
        ContainerInput::QuickMove => {
            let Some(slot) = usize::try_from(req.slot).ok() else {
                return Ok(());
            };
            let item = stack_in(world, layout[slot])
                .map(|stack| world.get::<ItemStack>(stack).unwrap().item());
            while quick_move(world, player, layout, slot, items)?
                && stack_in(world, layout[slot])
                    .and_then(|stack| world.get::<ItemStack>(stack))
                    .map(|stack| stack.item())
                    == item
            {}
        }
        ContainerInput::Pickup => {
            let Some(cell) = clicked_cell else {
                return Ok(());
            };
            match (stack_in(world, cell), carried) {
                (None, None) => {}
                (None, Some(carried)) => {
                    let amount = if primary { count(world, carried) } else { 1 };
                    safe_insert(world, carried, cell, amount, items)?;
                }
                (Some(clicked), None) => {
                    let have = count(world, clicked);
                    let amount = if primary { have } else { have.div_ceil(2) };
                    if let Some(taken) = take(world, clicked, amount, items) {
                        place(world, taken, carried_cell)?;
                    }
                }
                (Some(clicked), Some(carried)) => {
                    let same = same_item_same_components(world, clicked, carried, items);
                    match cell_max(world, cell, carried, items) {
                        Some(_) if same => {
                            let amount = if primary { count(world, carried) } else { 1 };
                            safe_insert(world, carried, cell, amount, items)?;
                        }
                        Some(max) if count(world, carried) <= max => {
                            mutate::detach(world, clicked);
                            place(world, carried, cell)?;
                            place(world, clicked, carried_cell)?;
                        }
                        Some(_) => {}
                        None if same => {
                            let max = max_stack_size(world.entity(carried), items);
                            if max - count(world, carried) >= count(world, clicked) {
                                mutate::merge_into(world, clicked, carried, max);
                            }
                        }
                        None => {}
                    }
                }
            }
        }
        ContainerInput::Swap => {
            let (Some(cell), Some(source_cell)) = (clicked_cell, swap_source(player, req.button))
            else {
                return Ok(());
            };
            match (stack_in(world, source_cell), stack_in(world, cell)) {
                (None, None) => {}
                (None, Some(target)) => place(world, target, source_cell)?,
                (Some(source), None) => {
                    if let Some(max) = cell_max(world, cell, source, items)
                        && let Some(moving) = take(world, source, max, items)
                    {
                        place(world, moving, cell)?;
                    }
                }
                (Some(source), Some(target)) => {
                    let Some(max) = cell_max(world, cell, source, items) else {
                        return Ok(());
                    };
                    mutate::detach(world, target);
                    if count(world, source) > max {
                        let moving =
                            mutate::split(world, source, max, items).expect("a live stack splits");
                        place(world, moving, cell)?;
                        insert_or_drop(world, player, target, items)?;
                    } else {
                        place(world, source, cell)?;
                        place(world, target, source_cell)?;
                    }
                }
            }
        }
        ContainerInput::Clone => {
            let creative = world
                .get::<PlayerGameMode>(player)
                .is_some_and(|mode| mode.0 == GameMode::Creative);
            let Some(clicked) = clicked_cell.and_then(|cell| stack_in(world, cell)) else {
                return Ok(());
            };
            if !creative || carried.is_some() {
                return Ok(());
            }
            let mut value = stack_to_value(world, clicked, items);
            value.count.0 = i32::from(max_stack_size(world.entity(clicked), items));
            let clone = mutate::spawn_stack(world, &value, items).expect("a live stack re-spawns");
            place(world, clone, carried_cell)?;
        }
        ContainerInput::Throw => {
            let Some(clicked) = clicked_cell.and_then(|cell| stack_in(world, cell)) else {
                return Ok(());
            };
            if carried.is_some() {
                return Ok(());
            }
            let amount = if primary { 1 } else { count(world, clicked) };
            if let Some(thrown) = take(world, clicked, amount, items) {
                drop_stack(world, player, thrown);
            }
        }
        ContainerInput::QuickCraft | ContainerInput::PickupAll => {}
    }
    Ok(())
}

fn swap_source(player: Entity, button: u8) -> Option<(Entity, u16)> {
    match button {
        0..=8 => Some((player, slots::held(button))),
        SWAP_OFFHAND_BUTTON => Some((player, slots::OFFHAND)),
        _ => None,
    }
}

/// ponytail: no drop spam throttle; a per-player counter (20 per drop, -1 per
/// tick, allowed below 1480) is the upgrade.
pub fn handle_creative_slots(world: &mut World) {
    let requests: Vec<CreativeSlotRequest> = world
        .resource_mut::<Messages<CreativeSlotRequest>>()
        .drain()
        .collect();
    if requests.is_empty() {
        return;
    }
    let items = world.resource::<Items>().clone();
    let registry = world.resource::<RegistryAccess>().clone();
    let blocks = world.resource::<Blocks>().clone();
    let lookups: [&dyn RegistryLookup; 2] = [&registry, &*blocks.0];
    let lookup = ChainLookup(&lookups);
    for req in requests {
        if !world
            .get::<PlayerGameMode>(req.player)
            .is_some_and(|mode| mode.0 == GameMode::Creative)
        {
            continue;
        }
        let resolved = req.item.resolve(&lookup).and_then(|slot| {
            (!slot.is_empty())
                .then(|| slot.to_value(&lookup))
                .transpose()
        });
        let value = match resolved {
            Ok(value) => value,
            Err(error) => {
                tracing::warn!(%error, player = ?req.player, "a creative stack was rejected");
                continue;
            }
        };
        let valid_slot = (1..=slots::OFFHAND as i16).contains(&req.slot);
        if !valid_slot && req.slot >= 0 {
            continue;
        }
        let existing = stack_in(world, (req.player, req.slot as u16)).filter(|_| valid_slot);
        let Some(value) = value else {
            if let Some(existing) = existing {
                mutate::set_count(world, existing, 0);
            }
            continue;
        };
        if let Some(existing) =
            existing.filter(|existing| stack_to_value(world, *existing, &items).item == value.item)
        {
            let over_max =
                value.count.0 > i32::from(max_stack_size(world.entity(existing), &items));
            if !over_max && let Err(error) = mutate::apply_value(world, existing, &value, &items) {
                tracing::warn!(%error, player = ?req.player, "a creative stack was rejected");
            }
            continue;
        }
        let stack = match mutate::spawn_stack(world, &value, &items) {
            Ok(stack) => stack,
            Err(error) => {
                tracing::warn!(%error, player = ?req.player, "a creative stack was rejected");
                continue;
            }
        };
        if value.count.0 > i32::from(max_stack_size(world.entity(stack), &items)) {
            world.despawn(stack);
            continue;
        }
        if !valid_slot {
            drop_stack(world, req.player, stack);
            continue;
        }
        if let Some(existing) = existing {
            mutate::set_count(world, existing, 0);
        }
        if let Err(error) = mutate::move_stack(world, stack, req.player, req.slot as u16) {
            tracing::warn!(%error, player = ?req.player, "a creative stack could not be placed");
            world.despawn(stack);
        }
    }
}

pub fn close_menus(world: &mut World) {
    let requests: Vec<CloseContainerRequest> = world
        .resource_mut::<Messages<CloseContainerRequest>>()
        .drain()
        .collect();
    if requests.is_empty() {
        return;
    }
    let items = world.resource::<Items>().clone();
    for req in requests {
        let Some(menu) = world
            .get::<CurrentMenu>(req.player)
            .map(|current| current.0)
        else {
            continue;
        };
        if world
            .get::<Menu>(menu)
            .is_none_or(|menu| i32::from(menu.container_id) != req.container_id)
        {
            continue;
        }
        if world.get::<MenuContainer>(menu).is_some() {
            close_container_menu(world, req.player, menu, false);
            continue;
        }
        let returning: Vec<Entity> = std::iter::once(slots::CARRIED)
            .chain(slots::CRAFT)
            .filter_map(|index| stack_in(world, (req.player, index)))
            .collect();
        for stack in returning {
            if let Err(error) = insert_or_drop(world, req.player, stack, &items) {
                tracing::debug!(%error, player = ?req.player, "a stack could not be returned on close");
            }
        }
    }
}

pub fn handle_drop_actions(world: &mut World, mut cursor: Local<MessageCursor<PlayerAction>>) {
    let actions: Vec<(Entity, PlayerActionKind)> = cursor
        .read(world.resource::<Messages<PlayerAction>>())
        .filter(|action| {
            matches!(
                action.kind,
                PlayerActionKind::DropItem
                    | PlayerActionKind::DropAllItems
                    | PlayerActionKind::SwapItemWithOffhand
            )
        })
        .map(|action| (action.player, action.kind.clone()))
        .collect();
    if actions.is_empty() {
        return;
    }
    let items = world.resource::<Items>().clone();
    for (player, kind) in actions {
        let Some((table, selected)) = world
            .get::<SlotTable>(player)
            .zip(world.get::<SelectedHotbarSlot>(player))
        else {
            continue;
        };
        let held_cell = (player, slots::held(selected.0));
        let held = held_stack(table, selected);
        match kind {
            PlayerActionKind::DropItem => {
                if let Some(thrown) = held.and_then(|held| take(world, held, 1, &items)) {
                    drop_stack(world, player, thrown);
                }
            }
            PlayerActionKind::DropAllItems => {
                if let Some(held) = held {
                    drop_stack(world, player, held);
                }
            }
            PlayerActionKind::SwapItemWithOffhand => {
                let offhand = stack_in(world, (player, slots::OFFHAND));
                if let Some(held) = held {
                    mutate::detach(world, held);
                }
                let swapped = offhand
                    .map_or(Ok(()), |offhand| place(world, offhand, held_cell))
                    .and_then(|()| {
                        held.map_or(Ok(()), |held| place(world, held, (player, slots::OFFHAND)))
                    });
                if let Err(error) = swapped {
                    tracing::debug!(%error, player = ?player, "the offhand swap failed");
                }
            }
            _ => {}
        }
    }
}

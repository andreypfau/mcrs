use bevy_ecs::change_detection::Mut;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::query::With;
use bevy_ecs::system::In;
use bevy_ecs::system::InMut;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use bevy_ecs::system::ResMut;
use bevy_ecs::world::World;
use mcrs_minecraft_container_click::Click;
use mcrs_minecraft_container_click::ClickKind;
use mcrs_minecraft_container_click::Clicked;
use mcrs_minecraft_engine_inventory_click::MenuClicker;
use mcrs_minecraft_engine_inventory_click::MenuClicks;
use mcrs_minecraft_engine_inventory_menu::MenuParts;
use mcrs_minecraft_engine_inventory_menu::Menus;
use mcrs_minecraft_engine_item::HolderCommandsExt;
use mcrs_minecraft_engine_item::ItemCommandsExt;
use mcrs_minecraft_engine_item::Request;
use mcrs_minecraft_engine_item::RequestDrop;
use mcrs_minecraft_engine_item::RequestId;
use mcrs_minecraft_engine_item::RequestResult;
use mcrs_minecraft_engine_item::Selected;
use mcrs_minecraft_engine_item::SlotRef;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_item::StackCommandsExt;
use mcrs_minecraft_engine_item::Stacks;
use mcrs_minecraft_engine_item::stack_capacity;
use mcrs_minecraft_engine_registries::RegistryNames;
use mcrs_minecraft_game_mode::Abilities;
use mcrs_minecraft_inventory_click::ClickPlayer;
use mcrs_minecraft_inventory_menu::MenuLayout;
use mcrs_minecraft_inventory_player::PlayerMode;
use mcrs_minecraft_inventory_transaction::Op;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::MaxStackSize;
use mcrs_minecraft_item_stack::PatchEntry;
use mcrs_minecraft_item_stack::write_wire_with_length;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_player_holders::PlayerHolders;
use mcrs_minecraft_protocol::Decode;
use mcrs_minecraft_protocol_game::item::DelimitedStack;
use mcrs_minecraft_protocol_game::serverbound::ServerboundSetCreativeModeSlot;

use crate::CreativeArea;
use crate::CreativeClick;
use crate::click_purpose::ClickPurpose;
use crate::click_source::ClickSource;
use crate::creative_drop_counter::CreativeDropCounter;
use crate::local_inventory::StackNames;
use crate::local_inventory::shows_inventory_menu;
use crate::outbox::Outbox;
use crate::outgoing::Outgoing;
use crate::predict_click::ClickScratch;

const ENTRIES: u16 = 46;
const HOTBAR: core::ops::Range<u16> = 36..45;
const HOTBAR_KEYS: u8 = 9;
const OFFHAND_BUTTON: u8 = 40;
const OFFHAND_ENTRY: u16 = 45;
const LEFT_BUTTON: u8 = 0;
const DROP_SLOT: i16 = -1;
const SLOT_LOCK_NAMESPACE: &str = "minecraft";
const SLOT_LOCK_PATH: &str = "creative_slot_lock";

#[derive(Default)]
pub(crate) struct CreativeScratch {
    entries: Vec<EntryBefore>,
    writes: Vec<SlotWrite>,
    drops: Vec<ItemStack>,
    moved: Vec<ItemStack>,
    drop_items: Vec<DelimitedStack>,
    dropped: Vec<Entity>,
    bytes: Vec<u8>,
}

struct EntryBefore {
    slot: Option<SlotRef>,
    stack: Option<ItemStack>,
}

struct SlotWrite {
    slot: SlotRef,
    stack: Option<ItemStack>,
}

#[expect(
    clippy::large_enum_variant,
    reason = "a plan lives on the stack for the one creative click that makes it"
)]
enum CreativePlan {
    Refused,
    Writes,
    Request(Request),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Committed {
    Applied,
    Refused,
}

pub(crate) fn predict_creative(world: &mut World, viewer: Entity, creative: CreativeClick) {
    world.try_resource_scope(|world, mut scratch: Mut<ClickScratch>| {
        let scratch = &mut scratch.creative;
        let plan = world.run_system_cached_with(plan_creative, (viewer, creative, &mut *scratch));
        match plan {
            Ok(CreativePlan::Writes) => write_slots(world, scratch),
            Ok(CreativePlan::Request(request)) => {
                if commit(world, viewer, request, scratch) == Committed::Refused {
                    return;
                }
            }
            Ok(CreativePlan::Refused) | Err(_) => return,
        }
        if !scratch.drops.is_empty()
            && let Some(mut counter) = world.get_mut::<CreativeDropCounter>(viewer)
        {
            counter.add_drops(scratch.drops.len());
        }
        world
            .run_system_cached_with(send_creative, &mut *scratch)
            .ok();
    });
}

fn write_slots(world: &mut World, scratch: &mut CreativeScratch) {
    for write in scratch.writes.drain(..) {
        let occupant = (world.get::<Slots>(write.slot.holder))
            .and_then(|slots| slots.get(usize::from(write.slot.index)));
        let mut commands = world.commands();
        if let Some(occupant) = occupant {
            commands.entity(occupant).despawn();
        }
        if let Some(stack) = write.stack {
            commands
                .spawn_stack(stack)
                .put_at(write.slot.holder, write.slot.index);
        }
        world.flush();
    }
}

fn commit(
    world: &mut World,
    viewer: Entity,
    request: Request,
    scratch: &mut CreativeScratch,
) -> Committed {
    let id = request.id();
    let mut cursor = world
        .get_resource::<Messages<RequestDrop>>()
        .map(Messages::get_cursor_current);
    let mut results = world
        .get_resource::<Messages<RequestResult>>()
        .map(Messages::get_cursor_current);
    world.commands().entity(viewer).request(request);
    world.flush();
    let applied = match (
        results.as_mut(),
        world.get_resource::<Messages<RequestResult>>(),
    ) {
        (Some(results), Some(messages)) => results
            .read(messages)
            .any(|result| result.holder == viewer && result.id == id && result.outcome.is_ok()),
        _ => false,
    };
    scratch.dropped.clear();
    if let (Some(cursor), Some(messages)) = (
        cursor.as_mut(),
        world.get_resource::<Messages<RequestDrop>>(),
    ) {
        let ours = cursor
            .read(messages)
            .filter(|drop| drop.holder == viewer && drop.id == id);
        scratch.dropped.extend(ours.map(|drop| drop.stack));
    }
    for stack in scratch.dropped.drain(..) {
        if let Ok(entity) = world.get_entity_mut(stack) {
            entity.despawn();
        }
    }
    if applied {
        Committed::Applied
    } else {
        Committed::Refused
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system: each query and resource is its own parameter"
)]
fn plan_creative(
    (In(viewer), In(creative), InMut(scratch)): (
        In<Entity>,
        In<CreativeClick>,
        InMut<CreativeScratch>,
    ),
    players: Query<
        (
            &PlayerHolders,
            Option<&Selected>,
            Option<&Abilities>,
            Option<&CreativeDropCounter>,
            &Menus,
        ),
        With<Slots>,
    >,
    menus: Query<(&MenuLayout, &MenuParts)>,
    holders: Query<&Slots>,
    mut stacks: Stacks,
    table: Option<Res<ItemTable>>,
    source: Option<Res<ClickSource>>,
    mut clicks: MenuClicks,
    names: StackNames,
) -> CreativePlan {
    scratch.entries.clear();
    scratch.writes.clear();
    scratch.drops.clear();
    scratch.moved.clear();
    scratch.drop_items.clear();
    let Ok((player_holders, selected, abilities, counter, player_menus)) = players.get(viewer)
    else {
        return CreativePlan::Refused;
    };
    let creative_flag = abilities.is_some_and(|abilities| abilities.instant_build);
    if !creative_flag || !shows_inventory_menu(player_menus) {
        return CreativePlan::Refused;
    }
    let (Some(table), Some(source), Some(menu)) = (table, source, player_menus.open()) else {
        return CreativePlan::Refused;
    };
    let Ok((layout, parts)) = menus.get(menu) else {
        return CreativePlan::Refused;
    };
    let occupant = |slot: SlotRef| {
        (holders.get(slot.holder).ok()).and_then(|slots| slots.get(usize::from(slot.index)))
    };
    for entry in 0..ENTRIES {
        let slot = layout.entry(entry).and_then(|entry| parts.slot(entry));
        let stack = (slot.and_then(occupant)).and_then(|stack| stacks.to_value(stack));
        scratch.entries.push(EntryBefore { slot, stack });
    }
    let cursor_slot = SlotRef {
        holder: player_holders.cursor,
        index: 0,
    };
    let cursor = occupant(cursor_slot).and_then(|stack| stacks.to_value(stack));
    let click = creative.click;
    let plan = match (creative.area, click.clicked) {
        (CreativeArea::Hotbar, clicked) if !matches!(clicked, Clicked::Entry(entry) if HOTBAR.contains(&entry)) =>
        {
            return CreativePlan::Refused;
        }
        (CreativeArea::Grid(shown), _) => {
            plan_grid(shown.as_ref(), cursor, click, &table, cursor_slot, scratch);
            CreativePlan::Writes
        }
        (CreativeArea::Hotbar, Clicked::Entry(entry))
            if click.kind == ClickKind::QuickMove && HOTBAR.contains(&entry) =>
        {
            empty_entry(scratch, entry);
            CreativePlan::Writes
        }
        (CreativeArea::Destroy, _) if click.kind == ClickKind::QuickMove => {
            for entry in 0..ENTRIES {
                empty_entry(scratch, entry);
            }
            CreativePlan::Writes
        }
        (CreativeArea::Destroy, _) => {
            scratch.writes.push(SlotWrite {
                slot: cursor_slot,
                stack: None,
            });
            CreativePlan::Writes
        }
        (CreativeArea::Hotbar | CreativeArea::Inventory, _) => {
            let clicker = MenuClicker {
                player: ClickPlayer {
                    mode: PlayerMode::Creative,
                    selected: selected.map_or(0, |selected| selected.0),
                },
                cursor: cursor_slot,
                request: RequestId {
                    source: source.0,
                    serial: ClickPurpose::Creative.serial(),
                },
            };
            let Ok(request) = clicks.plan(viewer, click, clicker) else {
                return CreativePlan::Refused;
            };
            note_request(&request, &holders, &mut stacks, scratch);
            CreativePlan::Request(request)
        }
    };
    let drops_allowed = counter.copied().unwrap_or_default().allows_drop();
    if !scratch.drops.is_empty() && !drops_allowed {
        return CreativePlan::Refused;
    }
    let Some(names) = names.names() else {
        return CreativePlan::Refused;
    };
    if !encodes(scratch, cursor_slot, &names) {
        return CreativePlan::Refused;
    }
    plan
}

fn note_request(
    request: &Request,
    holders: &Query<&Slots>,
    stacks: &mut Stacks,
    scratch: &mut CreativeScratch,
) {
    let moved = &mut scratch.moved;
    for op in request.ops() {
        match op {
            Op::Split { from, to, .. } | Op::Merge { from, to } => {
                moved.extend(held_at(holders, stacks, from));
                moved.extend(held_at(holders, stacks, to));
            }
            Op::Swap { left, right } => {
                moved.extend(held_at(holders, stacks, left));
                moved.extend(held_at(holders, stacks, right));
            }
            Op::Drop { from, count } => {
                let Some(held) = held_at(holders, stacks, from) else {
                    continue;
                };
                (scratch.drops).extend(with_count(&held, count.min(held.count.get())));
                moved.push(held);
            }
            Op::Set { slot, stack, .. } => {
                moved.extend(held_at(holders, stacks, slot));
                moved.extend(stack.and_then(|stack| stacks.to_value(stack)));
            }
        }
    }
}

fn held_at(holders: &Query<&Slots>, stacks: &mut Stacks, slot: SlotRef) -> Option<ItemStack> {
    let stack =
        (holders.get(slot.holder).ok()).and_then(|slots| slots.get(usize::from(slot.index)))?;
    stacks.to_value(stack)
}

fn encodes(scratch: &mut CreativeScratch, cursor_slot: SlotRef, names: &RegistryNames<'_>) -> bool {
    let CreativeScratch {
        writes,
        drops,
        moved,
        drop_items,
        bytes,
        ..
    } = scratch;
    for drop in drops.iter() {
        let Some(item) = delimited(Some(drop), names, bytes) else {
            return false;
        };
        drop_items.push(item);
    }
    if !(moved.iter()).all(|stack| delimited(Some(stack), names, bytes).is_some()) {
        return false;
    }
    let mut entry_writes = writes.iter().filter(|write| write.slot != cursor_slot);
    entry_writes.all(|write| delimited(write.stack.as_ref(), names, bytes).is_some())
}

fn plan_grid(
    shown: Option<&ItemStack>,
    cursor: Option<ItemStack>,
    click: Click,
    table: &ItemTable,
    cursor_slot: SlotRef,
    scratch: &mut CreativeScratch,
) {
    if shown.is_some_and(slot_locked) {
        return;
    }
    let full = |stack: &ItemStack| with_count(stack, capacity(table, stack));
    match click.kind {
        ClickKind::Swap => {
            let entry = match click.button {
                OFFHAND_BUTTON => Some(OFFHAND_ENTRY),
                key if key < HOTBAR_KEYS => Some(HOTBAR.start + u16::from(key)),
                _ => None,
            };
            let slot = entry
                .and_then(|entry| scratch.entries.get(usize::from(entry)))
                .and_then(|entry| entry.slot);
            if let (Some(slot), Some(shown)) = (slot, shown) {
                scratch.writes.push(SlotWrite {
                    slot,
                    stack: full(shown),
                });
            }
        }
        ClickKind::Clone => {
            if let (None, Some(shown)) = (cursor.as_ref(), shown) {
                scratch.writes.push(SlotWrite {
                    slot: cursor_slot,
                    stack: full(shown),
                });
            }
        }
        ClickKind::Throw => {
            if let Some(shown) = shown {
                let count = if click.button == LEFT_BUTTON {
                    1
                } else {
                    capacity(table, shown)
                };
                scratch.drops.extend(with_count(shown, count));
            }
        }
        ClickKind::PickUp | ClickKind::QuickMove => {
            let next = grid_cursor(shown, cursor.as_ref(), click, table);
            if next != cursor {
                scratch.writes.push(SlotWrite {
                    slot: cursor_slot,
                    stack: next,
                });
            }
        }
        ClickKind::Drag | ClickKind::PickUpAll => {}
    }
}

fn grid_cursor(
    shown: Option<&ItemStack>,
    cursor: Option<&ItemStack>,
    click: Click,
    table: &ItemTable,
) -> Option<ItemStack> {
    let quick_move = click.kind == ClickKind::QuickMove;
    let left = click.button == LEFT_BUTTON;
    match (cursor, shown) {
        (Some(cursor), Some(shown)) if cursor.mergeable(shown) => {
            let count = cursor.count.get();
            let max = capacity(table, cursor);
            if left && quick_move {
                with_count(cursor, max)
            } else if left && count < max {
                with_count(cursor, count.saturating_add(1))
            } else if left {
                Some(cursor.clone())
            } else {
                with_count(cursor, count.saturating_sub(1))
            }
        }
        (None, Some(shown)) => {
            let count = if quick_move {
                capacity(table, shown)
            } else {
                1
            };
            with_count(shown, count)
        }
        (Some(_), _) if left => None,
        (Some(cursor), _) => with_count(cursor, cursor.count.get().saturating_sub(1)),
        (None, None) => None,
    }
}

fn empty_entry(scratch: &mut CreativeScratch, entry: u16) {
    let Some(before) = scratch.entries.get(usize::from(entry)) else {
        return;
    };
    if let (Some(slot), Some(_)) = (before.slot, before.stack.as_ref()) {
        scratch.writes.push(SlotWrite { slot, stack: None });
    }
}

fn slot_locked(stack: &ItemStack) -> bool {
    stack.patch().iter().any(|entry| {
        matches!(entry, PatchEntry::Undecoded(value)
            if value.name.namespace() == SLOT_LOCK_NAMESPACE && value.name.path() == SLOT_LOCK_PATH)
    })
}

fn capacity(table: &ItemTable, stack: &ItemStack) -> u8 {
    stack_capacity(table.effective::<MaxStackSize>(stack))
}

fn with_count(stack: &ItemStack, count: u8) -> Option<ItemStack> {
    let mut stack = stack.clone();
    stack.count = Count::new(count)?;
    Some(stack)
}

fn send_creative(
    InMut(scratch): InMut<CreativeScratch>,
    holders: Query<&Slots>,
    mut stacks: Stacks,
    names: StackNames,
    outbox: Option<ResMut<Outbox>>,
) {
    let (Some(names), Some(mut outbox)) = (names.names(), outbox) else {
        return;
    };
    let CreativeScratch {
        entries,
        drop_items,
        bytes,
        ..
    } = scratch;
    for item in drop_items.drain(..) {
        outbox
            .0
            .push(Outgoing::CreativeSlot(ServerboundSetCreativeModeSlot {
                slot: DROP_SLOT,
                item,
            }));
    }
    for (index, before) in (0_i16..).zip(entries.iter()) {
        let Some(slot) = before.slot else {
            continue;
        };
        let after =
            (holders.get(slot.holder).ok()).and_then(|slots| slots.get(usize::from(slot.index)));
        let unchanged = match (before.stack.as_ref(), after) {
            (None, None) => true,
            (Some(value), Some(stack)) => stacks.equals_value(stack, value),
            (None, Some(_)) | (Some(_), None) => false,
        };
        if unchanged {
            continue;
        }
        let value = after.and_then(|stack| stacks.to_value(stack));
        if let Some(item) = delimited(value.as_ref(), &names, bytes) {
            outbox
                .0
                .push(Outgoing::CreativeSlot(ServerboundSetCreativeModeSlot {
                    slot: index,
                    item,
                }));
        }
    }
}

fn delimited(
    stack: Option<&ItemStack>,
    names: &RegistryNames<'_>,
    bytes: &mut Vec<u8>,
) -> Option<DelimitedStack> {
    bytes.clear();
    if let Err(error) = write_wire_with_length(stack, names, bytes) {
        tracing::warn!("a creative stack has no wire form: {error:?}");
        return None;
    }
    DelimitedStack::decode(&mut bytes.as_slice()).ok()
}

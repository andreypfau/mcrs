use mcrs_minecraft_inventory_player::Slot;
use mcrs_minecraft_inventory_player::Stacking;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::Damage;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::MaxStackSize;
use mcrs_minecraft_item_table::ItemTable;

use crate::close_place::Place;

pub(crate) fn insert_stack(
    inventory: &[Option<ItemStack>],
    selected: u8,
    stack: &ItemStack,
    table: &ItemTable,
    mut place: impl FnMut(u16) -> Place,
    mut moved: impl FnMut(u16, u8),
) -> u8 {
    let max = max_stack_size(stack, table);
    let stacking = if table
        .effective::<Damage>(stack)
        .is_some_and(|damage| damage.0 > 0)
    {
        Stacking::Alone
    } else {
        Stacking::Stacks
    };
    let mut rest = stack.count.get();
    let mut received: u64 = 0;
    while rest > 0 {
        let mut merged: u64 = 0;
        let end = mcrs_minecraft_inventory_player::insert(
            rest,
            max,
            stacking,
            selected,
            |index| {
                if received & bit(index) != 0 {
                    return Slot::Other;
                }
                slot(inventory, stack, place(index), max, index)
            },
            |index, count| {
                merged |= bit(index);
                moved(index, count);
            },
        );
        received |= merged;
        let Some(index) = end.empty else {
            return end.rest;
        };
        let taken = end.filled.min(capacity(place(index), max));
        if taken > 0 {
            moved(index, taken);
        }
        rest = end.rest.saturating_add(end.filled - taken);
        let bit = bit(index);
        if bit == 0 {
            return rest;
        }
        received |= bit;
    }
    0
}

pub(crate) fn max_stack_size(stack: &ItemStack, table: &ItemTable) -> u8 {
    Count::capacity(table.effective::<MaxStackSize>(stack))
}

fn slot(
    inventory: &[Option<ItemStack>],
    stack: &ItemStack,
    place: Place,
    max: u8,
    index: u16,
) -> Slot {
    if place == Place::Refuses {
        return Slot::Other;
    }
    match inventory.get(usize::from(index)) {
        Some(None) => Slot::Empty,
        Some(Some(held)) if held.mergeable(stack) => Slot::Alike {
            room: capacity(place, max).saturating_sub(held.count.get()),
        },
        _ => Slot::Other,
    }
}

fn capacity(place: Place, max: u8) -> u8 {
    match place {
        Place::Permits { limit } => limit.min(max),
        Place::Refuses => 0,
    }
}

fn bit(index: u16) -> u64 {
    1_u64.checked_shl(u32::from(index)).unwrap_or(0)
}

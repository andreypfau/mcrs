use mcrs_minecraft_inventory_player::Slot;
use mcrs_minecraft_inventory_player::Stacking;
use mcrs_minecraft_item_stack::Damage;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::MaxStackSize;
use mcrs_minecraft_item_table::ItemTable;

use crate::close_place::Place;

/// The largest value of the component `max_stack_size`.
const LARGEST_STACK: i32 = 99;

/// Gives `moved` each slot of `inventory`, whose selected hotbar slot is `selected`, that takes
/// items of `stack` by the insert rule, with the number of items, in the order of the rule
/// (INV-INS-001). The rule does not select a slot for which `place` refuses (INV-INS-003), and
/// fills each slot only up to its capacity (ITX-LIM-002). Changes no slot. Gives the number of
/// items that found no room.
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
    // The kernel fills an empty slot up to `max`, so each empty slot that the kernel selects
    // takes its capacity here, and the next pass sees it as `Slot::Other`.
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

/// The maximum stack size of `stack`, in the range `1..=99`.
pub(crate) fn max_stack_size(stack: &ItemStack, table: &ItemTable) -> u8 {
    clamp_stack_size(table.effective::<MaxStackSize>(stack))
}

/// The maximum stack size that the component `max_stack_size` gives, in the range `1..=99`. No
/// component gives 1.
pub(crate) fn clamp_stack_size(max: Option<&MaxStackSize>) -> u8 {
    let max = max.map_or(1, |max| max.0.clamp(1, LARGEST_STACK));
    u8::try_from(max).unwrap_or(1)
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

/// The bit of the slot `index` in a mask of slots, or 0 for an index from 64.
fn bit(index: u16) -> u64 {
    1_u64.checked_shl(u32::from(index)).unwrap_or(0)
}

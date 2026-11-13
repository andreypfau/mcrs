use mcrs_minecraft_item_stack::Damage;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::MaxStackSize;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_player_slots::Slot;
use mcrs_minecraft_player_slots::Stacking;

/// The largest value of the component `max_stack_size`.
const LARGEST_STACK: i32 = 99;

/// Gives `moved` each slot of `inventory`, whose selected hotbar slot is `selected`, that takes
/// items of `stack` by the insert rule, with the number of items, in the order of the rule
/// (INV-INS-001). Changes no slot. Gives the number of items that found no room.
pub(crate) fn insert_stack(
    inventory: &[Option<ItemStack>],
    selected: u8,
    stack: &ItemStack,
    table: &ItemTable,
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
    let end = mcrs_minecraft_player_slots::insert(
        stack.count.get(),
        max,
        stacking,
        selected,
        |index| slot(inventory, stack, max, index),
        &mut moved,
    );
    if let Some(index) = end.empty
        && end.filled > 0
    {
        moved(index, end.filled);
    }
    end.rest
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

fn slot(inventory: &[Option<ItemStack>], stack: &ItemStack, max: u8, index: u16) -> Slot {
    match inventory.get(usize::from(index)) {
        Some(None) => Slot::Empty,
        Some(Some(held)) if held.mergeable(stack) => Slot::Alike {
            room: max.saturating_sub(held.count.get()),
        },
        _ => Slot::Other,
    }
}

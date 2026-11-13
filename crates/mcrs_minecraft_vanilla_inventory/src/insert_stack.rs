use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::Damage;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::MaxStackSize;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_player_slots::Slot;
use mcrs_minecraft_player_slots::Stacking;

/// The largest value of the component `max_stack_size`.
const LARGEST_STACK: i32 = 99;

/// Puts the items of `stack` into `inventory`, whose selected hotbar slot is `selected`, by the
/// insert rule (INV-INS-001). Gives the items that found no room.
pub(crate) fn insert_stack(
    inventory: &mut [Option<ItemStack>],
    selected: u8,
    stack: ItemStack,
    table: &ItemTable,
) -> Option<ItemStack> {
    let max = table
        .effective::<MaxStackSize>(&stack)
        .map_or(1, |max| max.0.clamp(1, LARGEST_STACK));
    let max = u8::try_from(max).unwrap_or(1);
    let stacking = if table
        .effective::<Damage>(&stack)
        .is_some_and(|damage| damage.0 > 0)
    {
        Stacking::Alone
    } else {
        Stacking::Stacks
    };
    let mut merges = Vec::new();
    let end = mcrs_minecraft_player_slots::insert(
        stack.count.get(),
        max,
        stacking,
        selected,
        |index| slot(inventory, &stack, max, index),
        |index, moved| merges.push((index, moved)),
    );
    for (index, moved) in merges {
        if let Some(Some(held)) = inventory.get_mut(usize::from(index))
            && let Some(count) = held.count.get().checked_add(moved).and_then(Count::new)
        {
            held.count = count;
        }
    }
    if let (Some(index), Some(filled)) = (end.empty, Count::new(end.filled))
        && let Some(empty) = inventory.get_mut(usize::from(index))
    {
        let mut placed = stack.clone();
        placed.count = filled;
        *empty = Some(placed);
    }
    let rest = Count::new(end.rest)?;
    let mut rest_stack = stack;
    rest_stack.count = rest;
    Some(rest_stack)
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

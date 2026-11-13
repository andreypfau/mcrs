use core::mem;

use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_table::ItemTable;

use crate::insert_stack::insert_stack;
use crate::player_stacks::PlayerStacks;

/// Does the steps 1 and 2 of INV-CLS-001 for the inventory menu: the cursor stack, then each
/// stack of the craft grid from the slot 0 to the slot 3, enters the player inventory by the
/// insert rule. The cursor and the craft grid then hold the empty stack; so does the craft result,
/// which `stacks` does not hold. Gives the rest of each source stack that found no room, in that
/// order: each one becomes one item entity (IEN-SPN-006).
pub(crate) fn close_inventory_menu(stacks: &mut PlayerStacks, table: &ItemTable) -> Vec<ItemStack> {
    let cursor = stacks.cursor.take();
    let craft_grid = mem::take(&mut stacks.craft_grid);
    let mut rests = Vec::new();
    for stack in cursor.into_iter().chain(craft_grid.into_iter().flatten()) {
        if let Some(rest) = insert_stack(&mut stacks.inventory, stacks.selected, stack, table) {
            rests.push(rest);
        }
    }
    rests
}

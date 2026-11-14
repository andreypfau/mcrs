use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_table::ItemTable;

use crate::insert_stack::insert_stack;
use crate::player_stacks::CRAFT_GRID_SLOTS;
use crate::player_stacks::PlayerStacks;

/// A slot of the player whose stack a close moves into the player inventory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Carried {
    Cursor,
    CraftGrid(u16),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Destination {
    Inventory(u16),
    /// One item entity (IEN-SPN-006).
    Drop,
}

/// `count` items that leave `from` for `to`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CloseMove {
    pub(crate) from: Carried,
    pub(crate) to: Destination,
    pub(crate) count: u8,
}

/// The moves of the steps 1 and 2 of INV-CLS-001 for the inventory menu: the cursor stack, then
/// each stack of the craft grid from the slot 0 to the slot 3, enters the player inventory by the
/// insert rule, and the rest of each one drops. After the moves the cursor and the craft grid
/// hold the empty stack.
pub(crate) fn close_inventory_menu(stacks: &PlayerStacks, table: &ItemTable) -> Vec<CloseMove> {
    let mut after = stacks.clone();
    let mut moves = Vec::new();
    let craft_grid = (0..CRAFT_GRID_SLOTS).filter_map(|index| u16::try_from(index).ok());
    let sources = [Carried::Cursor]
        .into_iter()
        .chain(craft_grid.map(Carried::CraftGrid));
    for from in sources {
        let Some(Some(stack)) = carried(&mut after, from).cloned() else {
            continue;
        };
        let start = moves.len();
        let rest = insert_stack(
            &after.inventory,
            after.selected,
            &stack,
            table,
            |to, count| {
                moves.push(CloseMove {
                    from,
                    to: Destination::Inventory(to),
                    count,
                });
            },
        );
        if rest > 0 {
            moves.push(CloseMove {
                from,
                to: Destination::Drop,
                count: rest,
            });
        }
        if let Some(added) = moves.get(start..) {
            apply_close_moves(&mut after, added);
        }
    }
    moves
}

/// Applies `moves` to `stacks`. Gives the stack of each move to [`Destination::Drop`], in order.
pub(crate) fn apply_close_moves(stacks: &mut PlayerStacks, moves: &[CloseMove]) -> Vec<ItemStack> {
    let mut drops = Vec::new();
    for &CloseMove { from, to, count } in moves {
        let Some(Some(source)) = carried(stacks, from) else {
            continue;
        };
        let (Some(moved), Some(left)) = (Count::new(count), source.count.get().checked_sub(count))
        else {
            continue;
        };
        let mut part = source.clone();
        part.count = moved;
        match Count::new(left) {
            Some(left) => source.count = left,
            None => {
                if let Some(slot) = carried(stacks, from) {
                    *slot = None;
                }
            }
        }
        match to {
            Destination::Inventory(index) => {
                let Some(slot) = stacks.inventory.get_mut(usize::from(index)) else {
                    continue;
                };
                match slot {
                    None => *slot = Some(part),
                    Some(held) => {
                        if let Some(sum) = held.count.get().checked_add(count).and_then(Count::new)
                        {
                            held.count = sum;
                        }
                    }
                }
            }
            Destination::Drop => drops.push(part),
        }
    }
    drops
}

fn carried(stacks: &mut PlayerStacks, from: Carried) -> Option<&mut Option<ItemStack>> {
    match from {
        Carried::Cursor => Some(&mut stacks.cursor),
        Carried::CraftGrid(index) => stacks.craft_grid.get_mut(usize::from(index)),
    }
}

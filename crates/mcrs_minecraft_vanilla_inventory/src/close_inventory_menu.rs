use mcrs_minecraft_inventory_file::PlayerItems;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_table::ItemTable;

use crate::close_place::Place;
use crate::insert_stack::insert_stack;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Carried {
    Cursor,
    CraftGrid(u16),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Destination {
    Inventory(u16),
    Drop,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CloseMove {
    pub(crate) from: Carried,
    pub(crate) to: Destination,
    pub(crate) count: u8,
}

pub(crate) fn close_inventory_menu(
    stacks: &PlayerItems,
    table: &ItemTable,
    mut place: impl FnMut(Carried, u16) -> Place,
) -> Vec<CloseMove> {
    let mut after = stacks.clone();
    let mut moves = Vec::new();
    let craft_grid = (0..stacks.craft_grid.len()).filter_map(|index| u16::try_from(index).ok());
    let sources = [Carried::Cursor]
        .into_iter()
        .chain(craft_grid.map(Carried::CraftGrid));
    for from in sources {
        let Some(Some(stack)) = carried(&mut after, from).cloned() else {
            continue;
        };
        let start = moves.len();
        let rest = insert_stack(
            &after.slots,
            after.selected,
            &stack,
            table,
            |to| place(from, to),
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

pub(crate) fn apply_close_moves(stacks: &mut PlayerItems, moves: &[CloseMove]) -> Vec<ItemStack> {
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
                let Some(slot) = stacks.slots.get_mut(usize::from(index)) else {
                    continue;
                };
                match slot {
                    None => *slot = Some(part),
                    Some(held) => match held.count.get().checked_add(count).and_then(Count::new) {
                        Some(sum) => held.count = sum,
                        None => drops.push(part),
                    },
                }
            }
            Destination::Drop => drops.push(part),
        }
    }
    drops
}

fn carried(stacks: &mut PlayerItems, from: Carried) -> Option<&mut Option<ItemStack>> {
    match from {
        Carried::Cursor => Some(&mut stacks.cursor),
        Carried::CraftGrid(index) => stacks.craft_grid.get_mut(usize::from(index)),
    }
}

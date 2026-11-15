use bevy_ecs::system::Query;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_item::Stacks;
use mcrs_minecraft_inventory_file::PlayerItems;
use mcrs_minecraft_item_stack::ItemStack;

use crate::PlayerHolders;

/// The stacks that the holders of a player hold now: `inventory` and, with `holders`, the cursor
/// and the craft grid.
pub(crate) fn from_holders(
    inventory: &Slots,
    selected: u8,
    holders: Option<&PlayerHolders>,
    holder_slots: &Query<&Slots>,
    stacks: &mut Stacks,
) -> PlayerItems {
    let mut items = PlayerItems {
        selected,
        ..PlayerItems::default()
    };
    read_slots(stacks, inventory, &mut items.slots);
    if let Some(holders) = holders {
        if let Ok(cursor) = holder_slots.get(holders.cursor) {
            items.cursor = cursor.get(0).and_then(|stack| stacks.to_value(stack));
        }
        if let Ok(craft_grid) = holder_slots.get(holders.craft_grid) {
            read_slots(stacks, craft_grid, &mut items.craft_grid);
        }
    }
    items
}

fn read_slots(stacks: &mut Stacks, slots: &Slots, values: &mut [Option<ItemStack>]) {
    for (value, stack) in values.iter_mut().zip(slots.iter()) {
        *value = stack.and_then(|stack| stacks.to_value(stack));
    }
}

use bevy_ecs::entity::Entity;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_inventory_file::PlayerItems;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_player_holders::PlayerHolders;

pub(crate) fn from_holders<'a>(
    inventory: &Slots,
    selected: u8,
    holders: Option<&PlayerHolders>,
    holder_slots: impl Fn(Entity) -> Option<&'a Slots>,
    mut stack_value: impl FnMut(Entity) -> Option<ItemStack>,
) -> PlayerItems {
    let mut items = PlayerItems {
        selected,
        ..PlayerItems::default()
    };
    read_slots(&mut stack_value, inventory, &mut items.slots);
    if let Some(holders) = holders {
        if let Some(cursor) = holder_slots(holders.cursor) {
            items.cursor = cursor.get(0).and_then(&mut stack_value);
        }
        if let Some(craft_grid) = holder_slots(holders.craft_grid) {
            read_slots(&mut stack_value, craft_grid, &mut items.craft_grid);
        }
    }
    items
}

fn read_slots(
    stack_value: &mut impl FnMut(Entity) -> Option<ItemStack>,
    slots: &Slots,
    values: &mut [Option<ItemStack>],
) {
    for (value, stack) in values.iter_mut().zip(slots.iter()) {
        *value = stack.and_then(&mut *stack_value);
    }
}

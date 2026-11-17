use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::MaxStackSize;
use mcrs_minecraft_item_table::ItemTable;

const LARGEST_STACK: i32 = 99;

pub(crate) fn max_stack_size(stack: &ItemStack, table: &ItemTable) -> u8 {
    clamp_stack_size(table.effective::<MaxStackSize>(stack))
}

pub(crate) fn clamp_stack_size(max: Option<&MaxStackSize>) -> u8 {
    let max = max.map_or(1, |max| max.0.clamp(1, LARGEST_STACK));
    u8::try_from(max).unwrap_or(1)
}

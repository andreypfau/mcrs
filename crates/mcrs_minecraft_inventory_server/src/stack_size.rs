use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::MaxStackSize;
use mcrs_minecraft_item_table::ItemTable;

/// The largest value of the component `max_stack_size`.
const LARGEST_STACK: i32 = 99;

// chisle: the edge and the gameplay each keep a copy of the clamp of `max_stack_size`; an
// accessor of the maximum stack size in the item table crate lifts it.
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

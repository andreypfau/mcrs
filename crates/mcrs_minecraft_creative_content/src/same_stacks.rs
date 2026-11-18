use alloc::vec;
use alloc::vec::Vec;

use mcrs_minecraft_item_stack::ItemStack;

pub(crate) fn remove_same_stacks(stacks: &mut Vec<ItemStack>) {
    let mut order: Vec<usize> = (0..stacks.len()).collect();
    order.sort_by_key(|&index| stacks.get(index).map(|stack| stack.kind));
    let mut keep = vec![true; stacks.len()];
    for item in order.chunk_by(|left, right| same_kind(stacks, *left, *right)) {
        for (at, &index) in item.iter().enumerate() {
            let Some(stack) = stacks.get(index) else {
                continue;
            };
            let earlier = item.get(..at).unwrap_or_default();
            let repeated = earlier.iter().any(|&other| {
                keep.get(other).copied().unwrap_or_default()
                    && stacks
                        .get(other)
                        .is_some_and(|other| other.mergeable(stack))
            });
            if repeated && let Some(flag) = keep.get_mut(index) {
                *flag = false;
            }
        }
    }
    let mut flags = keep.into_iter();
    stacks.retain(|_| flags.next().unwrap_or(true));
}

fn same_kind(stacks: &[ItemStack], left: usize, right: usize) -> bool {
    stacks.get(left).map(|stack| stack.kind) == stacks.get(right).map(|stack| stack.kind)
}

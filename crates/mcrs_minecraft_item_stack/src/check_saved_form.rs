use core::ops::RangeInclusive;

use crate::Count;
use crate::ItemStack;
use crate::NameLookup;
use crate::WireError;
use crate::nbt_depth::check_nbt_depth;
use crate::saved_form::Fault;

pub(crate) const NOT_NEGATIVE: RangeInclusive<i32> = 0..=i32::MAX;
pub(crate) const POSITIVE: RangeInclusive<i32> = 1..=i32::MAX;
pub(crate) const STACK_SIZE: RangeInclusive<i32> = 1..=Count::MAX as i32;
pub(crate) const OMINOUS_AMPLIFIER: RangeInclusive<i32> = 0..=4;

pub(crate) fn check_saved_form<L: NameLookup>(
    stack: &ItemStack,
    names: &L,
) -> Result<(), WireError> {
    check_nbt_depth(stack, names).map_err(Fault::into_wire)
}

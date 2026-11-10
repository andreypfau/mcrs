use core::num::NonZeroU8;

use crate::EnchantmentId;

/// One enchantment of a stack with its level. The level is never zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EnchantmentLevel {
    pub enchantment: EnchantmentId,
    pub level: NonZeroU8,
}

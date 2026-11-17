use core::num::NonZeroU8;

use crate::EnchantmentId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EnchantmentLevel {
    pub enchantment: EnchantmentId,
    pub level: NonZeroU8,
}

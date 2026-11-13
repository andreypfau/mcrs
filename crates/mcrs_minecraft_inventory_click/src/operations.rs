use mcrs_minecraft_inventory_plan::MAX_OPS;
use mcrs_minecraft_inventory_plan::Op;
use mcrs_minecraft_inventory_plan::Planned;

use crate::ClickError;

/// The operations of the request of one click. The caller owns it and uses it again for the
/// next click; it holds every operation inline.
#[derive(Clone, Debug)]
pub struct Operations<I, T> {
    ops: [Option<Op<I, Planned<T>>>; MAX_OPS],
    len: usize,
}

impl<I, T> Operations<I, T> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            ops: [const { None }; MAX_OPS],
            len: 0,
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub(crate) fn clear(&mut self) {
        self.len = 0;
    }

    pub(crate) fn push(&mut self, op: Op<I, Planned<T>>) -> Result<(), ClickError> {
        let free = self.ops.get_mut(self.len).ok_or(ClickError::TooManyOps)?;
        *free = Some(op);
        self.len = self.len.saturating_add(1);
        Ok(())
    }
}

impl<I: Copy, T: Copy> Operations<I, T> {
    /// The operations in the order of the request.
    pub fn iter(&self) -> impl Iterator<Item = Op<I, Planned<T>>> + '_ {
        self.ops.iter().take(self.len).flatten().copied()
    }
}

impl<I: Copy + PartialEq, T: Copy> Operations<I, T> {
    /// The number of items that the `split` operations so far take from `slot`.
    pub(crate) fn split_from(&self, slot: I) -> u8 {
        let mut taken: u8 = 0;
        for op in self.iter() {
            if let Op::Split { from, count, .. } = op
                && from == slot
            {
                taken = taken.saturating_add(count);
            }
        }
        taken
    }
}

impl<I, T> Default for Operations<I, T> {
    fn default() -> Self {
        Self::new()
    }
}

use std::io::Write;
use derive_more::{From, Into};
use mcrs_nbt::compound::NbtCompound;
use crate::{Decode, Encode};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash, Debug, From, Into)]
pub struct ItemId(pub u16);

/// A stack of items in an inventory.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct ItemStack {
    pub id: ItemId,
    pub count: i8,
    pub components: Option<NbtCompound>,
}

impl ItemStack {
    pub const EMPTY: ItemStack = ItemStack {
        id: ItemId(0),
        count: 0,
        components: None,
    };

    #[must_use]
    pub const fn new(item: ItemId, count: i8, nbt: Option<NbtCompound>) -> Self {
        Self { id: item, count, components: nbt }
    }

    #[must_use]
    pub const fn with_count(mut self, count: i8) -> Self {
        self.count = count;
        self
    }

    #[must_use]
    pub const fn with_item(mut self, item: ItemId) -> Self {
        self.id = item;
        self
    }

    #[must_use]
    pub fn with_nbt(mut self, nbt: impl Into<Option<NbtCompound>>) -> Self {
        self.components = nbt.into();
        self
    }

    pub const fn is_empty(&self) -> bool {
        self.id.0 == 0 || self.count <= 0
    }
}

impl Encode for ItemStack {
    fn encode(&self, mut w: impl Write) -> anyhow::Result<()> {
        if self.is_empty() {
            false.encode(w)
        } else {
            true.encode(&mut w)?;
            self.id.0.encode(&mut w)?;
            self.count.encode(&mut w)?;
            match &self.components {
                Some(n) => n.encode(w),
                None => 0u8.encode(w),
            }
        }
    }
}

impl Decode<'_> for ItemStack {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        let present = bool::decode(r)?;
        if !present {
            return Ok(ItemStack::EMPTY);
        };

        let item = ItemId(u16::decode(r)?);
        let count = i8::decode(r)?;

        let nbt = if let [0, rest @ ..] = *r {
            *r = rest;
            None
        } else {
            Some(NbtCompound::decode(r)?)
        };

        let stack = ItemStack { id: item, count, components: nbt };

        // Normalize empty item stacks.
        if stack.is_empty() {
            Ok(ItemStack::EMPTY)
        } else {
            Ok(stack)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_item_stack_is_empty() {
        let air_stack = ItemStack::new(ItemId::default(), 10, None);
        let less_then_one_stack = ItemStack::new(ItemId(1), 0, None);

        assert!(air_stack.is_empty());
        assert!(less_then_one_stack.is_empty());

        assert!(ItemStack::EMPTY.is_empty());

        let not_empty_stack = ItemStack::new(ItemId(1), 10, None);

        assert!(!not_empty_stack.is_empty());
    }
}

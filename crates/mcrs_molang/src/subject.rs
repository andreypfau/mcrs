use alloc::vec;
use alloc::vec::Vec;

use crate::layout::{Bank, VariableLayout, VariableSlot};
use crate::query::QuerySlot;

/// The `variable.` store of one subject. A slot with no assignment is missing and reads as
/// `0.0`.
#[derive(Clone, Debug, PartialEq)]
pub struct Variables {
    pub(crate) numbers: Vec<f32>,
    pub(crate) number_set: Vec<bool>,
    pub(crate) bits: Vec<u64>,
    pub(crate) bits_set: Vec<bool>,
}

impl Variables {
    /// An empty store for `layout`, with each variable missing.
    #[must_use]
    pub fn new(layout: &VariableLayout) -> Self {
        Self {
            numbers: vec![0.0; layout.number_slots()],
            number_set: vec![false; layout.number_slots()],
            bits: vec![0; layout.bits_slots()],
            bits_set: vec![false; layout.bits_slots()],
        }
    }

    /// The raw value of the slot, or `None` when it is missing. A number is its `f32` bits.
    #[must_use]
    pub fn get(&self, slot: VariableSlot) -> Option<u64> {
        let index = usize::from(slot.index);
        match slot.bank {
            Bank::Number => self
                .number_set
                .get(index)
                .copied()
                .unwrap_or(false)
                .then(|| {
                    self.numbers
                        .get(index)
                        .map(|value| u64::from(value.to_bits()))
                })
                .flatten(),
            Bank::Bits => self
                .bits_set
                .get(index)
                .copied()
                .unwrap_or(false)
                .then(|| self.bits.get(index).copied())
                .flatten(),
        }
    }

    /// Stores a raw value, or makes the slot missing with `None`.
    pub fn set(&mut self, slot: VariableSlot, raw: Option<u64>) {
        let index = usize::from(slot.index);
        match slot.bank {
            Bank::Number => {
                if let (Some(value), Some(set)) =
                    (self.numbers.get_mut(index), self.number_set.get_mut(index))
                {
                    *value = raw.map_or(0.0, |raw| f32::from_bits(raw as u32));
                    *set = raw.is_some();
                }
            }
            Bank::Bits => {
                if let (Some(value), Some(set)) =
                    (self.bits.get_mut(index), self.bits_set.get_mut(index))
                {
                    *value = raw.unwrap_or(0);
                    *set = raw.is_some();
                }
            }
        }
    }
}

/// The query and context values of one subject for one run, by slot. A number is its `f32`
/// bits. A slot with no value is missing.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct QueryValues {
    pub(crate) raw: Vec<u64>,
    pub(crate) set: Vec<bool>,
}

impl QueryValues {
    /// `count` missing slots.
    #[must_use]
    pub fn new(count: usize) -> Self {
        Self {
            raw: vec![0; count],
            set: vec![false; count],
        }
    }

    /// Gives the slot a raw value, or makes it missing with `None`.
    pub fn set(&mut self, slot: QuerySlot, raw: Option<u64>) {
        let index = usize::from(slot.0);
        if index >= self.raw.len() {
            self.raw.resize(index + 1, 0);
            self.set.resize(index + 1, false);
        }
        if let (Some(value), Some(set)) = (self.raw.get_mut(index), self.set.get_mut(index)) {
            *value = raw.unwrap_or(0);
            *set = raw.is_some();
        }
    }

    /// Gives the slot a number.
    pub fn set_number(&mut self, slot: QuerySlot, value: f32) {
        self.set(slot, Some(u64::from(value.to_bits())));
    }

    /// The raw value of the slot, or `None` when it is missing.
    #[must_use]
    pub fn get(&self, slot: QuerySlot) -> Option<u64> {
        let index = usize::from(slot.0);
        let set = self.set.get(index).copied().unwrap_or(false);
        self.raw.get(index).copied().filter(|_| set)
    }
}

/// The inputs and the variable store of one run.
#[derive(Debug)]
pub struct Subject<'a> {
    pub variables: &'a mut Variables,
    pub queries: &'a QueryValues,
    /// The value of `this`; `0.0` when the consumer supplies none.
    pub this: f32,
}

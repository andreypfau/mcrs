use alloc::vec;
use alloc::vec::Vec;

use crate::layout::{Bank, VariableLayout, VariableSlot};
use crate::query::QuerySlot;
use crate::value::ValueType;

/// The `variable.` stores of the lanes of one block, one column for each slot.
#[derive(Clone, Debug, PartialEq)]
pub struct BatchVariables<const N: usize> {
    pub(crate) numbers: Vec<[f32; N]>,
    pub(crate) number_set: Vec<[bool; N]>,
    pub(crate) bits: Vec<[u64; N]>,
    pub(crate) bits_set: Vec<[bool; N]>,
}

impl<const N: usize> BatchVariables<N> {
    /// Stores with each variable of each lane missing.
    #[must_use]
    pub fn new(layout: &VariableLayout) -> Self {
        Self {
            numbers: vec![[0.0; N]; layout.number_slots()],
            number_set: vec![[false; N]; layout.number_slots()],
            bits: vec![[0; N]; layout.bits_slots()],
            bits_set: vec![[false; N]; layout.bits_slots()],
        }
    }

    /// The raw value of a slot of a lane, or `None` when it is missing.
    #[must_use]
    pub fn get(&self, lane: usize, slot: VariableSlot) -> Option<u64> {
        let index = usize::from(slot.index);
        match slot.bank {
            Bank::Number => {
                let set = self.number_set.get(index)?.get(lane)?;
                let value = self.numbers.get(index)?.get(lane)?;
                set.then_some(u64::from(value.to_bits()))
            }
            Bank::Bits => {
                let set = self.bits_set.get(index)?.get(lane)?;
                let value = self.bits.get(index)?.get(lane)?;
                set.then_some(*value)
            }
        }
    }

    /// Stores a raw value in a slot of a lane, or makes it missing with `None`.
    pub fn set(&mut self, lane: usize, slot: VariableSlot, raw: Option<u64>) {
        let index = usize::from(slot.index);
        match slot.bank {
            Bank::Number => {
                if let Some(value) = self.numbers.get_mut(index).and_then(|c| c.get_mut(lane)) {
                    *value = raw.map_or(0.0, |raw| f32::from_bits(raw as u32));
                }
                if let Some(set) = self.number_set.get_mut(index).and_then(|c| c.get_mut(lane)) {
                    *set = raw.is_some();
                }
            }
            Bank::Bits => {
                if let Some(value) = self.bits.get_mut(index).and_then(|c| c.get_mut(lane)) {
                    *value = raw.unwrap_or(0);
                }
                if let Some(set) = self.bits_set.get_mut(index).and_then(|c| c.get_mut(lane)) {
                    *set = raw.is_some();
                }
            }
        }
    }
}

/// The query values of the lanes of one block, one column for each slot.
#[derive(Clone, Debug, PartialEq)]
pub struct BatchQueries<const N: usize> {
    pub(crate) numbers: Vec<[f32; N]>,
    pub(crate) bits: Vec<[u64; N]>,
    pub(crate) set: Vec<[bool; N]>,
}

impl<const N: usize> BatchQueries<N> {
    /// `count` slots, missing in each lane.
    #[must_use]
    pub fn new(count: usize) -> Self {
        Self {
            numbers: vec![[0.0; N]; count],
            bits: vec![[0; N]; count],
            set: vec![[false; N]; count],
        }
    }

    /// Gives a slot of a lane a raw value, or makes it missing with `None`.
    pub fn set(&mut self, lane: usize, slot: QuerySlot, raw: Option<u64>) {
        let index = usize::from(slot.0);
        if index >= self.set.len() {
            self.numbers.resize(index + 1, [0.0; N]);
            self.bits.resize(index + 1, [0; N]);
            self.set.resize(index + 1, [false; N]);
        }
        let raw_value = raw.unwrap_or(0);
        if let Some(value) = self.numbers.get_mut(index).and_then(|c| c.get_mut(lane)) {
            *value = f32::from_bits(raw_value as u32);
        }
        if let Some(value) = self.bits.get_mut(index).and_then(|c| c.get_mut(lane)) {
            *value = raw_value;
        }
        if let Some(set) = self.set.get_mut(index).and_then(|c| c.get_mut(lane)) {
            *set = raw.is_some();
        }
    }

    /// Gives a slot of a lane a number.
    pub fn set_number(&mut self, lane: usize, slot: QuerySlot, value: f32) {
        self.set(lane, slot, Some(u64::from(value.to_bits())));
    }

    /// The raw value of a slot of a lane, or `None` when it is missing.
    #[must_use]
    pub fn get(&self, lane: usize, slot: QuerySlot) -> Option<u64> {
        let index = usize::from(slot.0);
        let set = *self.set.get(index)?.get(lane)?;
        let raw = *self.bits.get(index)?.get(lane)?;
        set.then_some(raw)
    }
}

/// The inputs and stores of the lanes of one block.
#[derive(Debug)]
pub struct BatchSubjects<'a, const N: usize> {
    pub variables: &'a mut BatchVariables<N>,
    pub queries: &'a BatchQueries<N>,
    pub this: &'a [f32; N],
    /// The number of lanes that hold a subject, from the first lane; at most `N`.
    /// [`BatchVm::run_masked`](crate::BatchVm::run_masked) takes a mask instead.
    pub lanes: usize,
}

/// The results of one batch run.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BatchOutput<'a, const N: usize> {
    Numbers(&'a [f32; N]),
    /// The raw 64-bit results of a program with a string or resource result.
    Bits(&'a [u64; N], ValueType),
}

use alloc::boxed::Box;
use alloc::vec::Vec;

use hashbrown::HashMap;

use crate::value::ValueType;

/// An index of a `variable.` name within the bank of its type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct VariableSlot {
    pub index: u16,
    pub bank: Bank,
}

/// The storage of a value: `f32` numbers, or 64-bit strings and resources.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Bank {
    Number,
    Bits,
}

impl From<ValueType> for Bank {
    fn from(value_type: ValueType) -> Self {
        if value_type.is_bits() {
            Self::Bits
        } else {
            Self::Number
        }
    }
}

/// The dense slots of the `variable.` names of one host type. Each subject of the type stores
/// its variables in this order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VariableLayout {
    names: Vec<(Box<str>, ValueType)>,
    by_name: HashMap<Box<str>, (VariableSlot, ValueType)>,
    numbers: u16,
    bits: u16,
}

/// A variable that already has a slot of another type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TypeConflict {
    pub declared: ValueType,
}

impl VariableLayout {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn slot(&self, name: &str) -> Option<VariableSlot> {
        self.by_name.get(name).map(|(slot, _)| *slot)
    }

    #[must_use]
    pub fn value_type(&self, name: &str) -> Option<ValueType> {
        self.by_name.get(name).map(|(_, value_type)| *value_type)
    }

    /// The slot of `name`, made with `value_type` when the name has none.
    pub fn declare(
        &mut self,
        name: &str,
        value_type: ValueType,
    ) -> Result<VariableSlot, TypeConflict> {
        if let Some((slot, declared)) = self.by_name.get(name) {
            if *declared != value_type {
                return Err(TypeConflict {
                    declared: *declared,
                });
            }
            return Ok(*slot);
        }
        let bank = Bank::from(value_type);
        let counter = match bank {
            Bank::Number => &mut self.numbers,
            Bank::Bits => &mut self.bits,
        };
        let index = *counter;
        *counter = counter.checked_add(1).ok_or(TypeConflict {
            declared: value_type,
        })?;
        let slot = VariableSlot { index, bank };
        self.names.push((name.into(), value_type));
        self.by_name.insert(name.into(), (slot, value_type));
        Ok(slot)
    }

    /// The number of slots in the `f32` bank.
    #[must_use]
    pub const fn number_slots(&self) -> usize {
        self.numbers as usize
    }

    /// The number of slots in the 64-bit bank.
    #[must_use]
    pub const fn bits_slots(&self) -> usize {
        self.bits as usize
    }

    /// Each name with its slot and type, in declaration order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, VariableSlot, ValueType)> {
        self.names.iter().filter_map(|(name, value_type)| {
            self.by_name
                .get(name)
                .map(|(slot, _)| (&**name, *slot, *value_type))
        })
    }
}

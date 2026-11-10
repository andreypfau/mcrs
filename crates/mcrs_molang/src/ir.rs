//! A typed SSA form with structured control flow. A value is defined once. An `If` defines its
//! results from the yields of the region that runs, so `&&`, `||`, `?:` and `??` keep their
//! right sides lazy.

use alloc::vec::Vec;

use crate::diagnostic::Span;
use crate::layout::VariableSlot;
use crate::math::{BinaryFn, TernaryFn, UnaryFn};
use crate::query::QuerySlot;
use crate::value::{Rate, ValueType};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct ValueId(pub u32);

impl ValueId {
    pub(crate) const fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Op {
    Const(f32),
    ConstBits(u64),
    This,
    Query(QuerySlot),
    /// `1.0` when the query slot is missing.
    QueryMissing(QuerySlot),
    LoadVar(VariableSlot),
    /// `1.0` when the variable is missing.
    VarMissing(VariableSlot),
    StoreVar {
        slot: VariableSlot,
        value: ValueId,
        missing: ValueId,
    },
    /// Reads the missing flag of the run as `1.0` or `0.0`, and clears it.
    MissTake,
    /// Sets the missing flag of the run when the operand is truthy.
    MissOr(ValueId),
    Unary(UnaryFn, ValueId),
    Binary(BinaryFn, ValueId, ValueId),
    Ternary(TernaryFn, ValueId, ValueId, ValueId),
    /// `==` (or `!=` when `negate`) of two 64-bit values.
    CompareBits {
        negate: bool,
        a: ValueId,
        b: ValueId,
    },
    Select(ValueId, ValueId, ValueId),
    Table(u16, ValueId),
    If {
        condition: ValueId,
        then: Region,
        otherwise: Region,
        results: Vec<ValueId>,
    },
    /// A result of an `If`.
    Phi,
    Return(ValueId),
}

impl Op {
    /// The operands, except the contents of the regions of an `If`.
    pub(crate) fn operands(&self) -> Operands {
        let mut operands = Operands::default();
        match self {
            Self::Const(_)
            | Self::ConstBits(_)
            | Self::This
            | Self::Query(_)
            | Self::QueryMissing(_)
            | Self::LoadVar(_)
            | Self::VarMissing(_)
            | Self::MissTake
            | Self::Phi => {}
            Self::MissOr(a) | Self::Unary(_, a) | Self::Table(_, a) | Self::Return(a) => {
                operands.push(*a);
            }
            Self::StoreVar { value, missing, .. } => {
                operands.push(*value);
                operands.push(*missing);
            }
            Self::Binary(_, a, b) | Self::CompareBits { a, b, .. } => {
                operands.push(*a);
                operands.push(*b);
            }
            Self::Ternary(_, a, b, c) | Self::Select(a, b, c) => {
                operands.push(*a);
                operands.push(*b);
                operands.push(*c);
            }
            Self::If { condition, .. } => operands.push(*condition),
        }
        operands
    }

    pub(crate) fn map_operands(&mut self, mut map: impl FnMut(ValueId) -> ValueId) {
        match self {
            Self::Const(_)
            | Self::ConstBits(_)
            | Self::This
            | Self::Query(_)
            | Self::QueryMissing(_)
            | Self::LoadVar(_)
            | Self::VarMissing(_)
            | Self::MissTake
            | Self::Phi => {}
            Self::MissOr(a) | Self::Unary(_, a) | Self::Table(_, a) | Self::Return(a) => {
                *a = map(*a);
            }
            Self::StoreVar { value, missing, .. } => {
                *value = map(*value);
                *missing = map(*missing);
            }
            Self::Binary(_, a, b) | Self::CompareBits { a, b, .. } => {
                *a = map(*a);
                *b = map(*b);
            }
            Self::Ternary(_, a, b, c) | Self::Select(a, b, c) => {
                *a = map(*a);
                *b = map(*b);
                *c = map(*c);
            }
            Self::If { condition, .. } => *condition = map(*condition),
        }
    }
}

/// Up to three operands, with no allocation.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Operands {
    values: [ValueId; 3],
    count: usize,
}

impl Operands {
    fn push(&mut self, value: ValueId) {
        if let Some(slot) = self.values.get_mut(self.count) {
            *slot = value;
            self.count += 1;
        }
    }

    pub(crate) fn as_slice(&self) -> &[ValueId] {
        self.values.get(..self.count).unwrap_or_default()
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Region {
    pub insts: Vec<ValueId>,
    pub yields: Vec<ValueId>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ValueData {
    pub op: Op,
    pub value_type: ValueType,
    pub rate: Rate,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Function {
    pub values: Vec<ValueData>,
    pub body: Region,
    pub tables: Vec<Vec<u64>>,
    pub output: ValueType,
}

impl Function {
    pub(crate) fn value(&self, id: ValueId) -> Option<&ValueData> {
        self.values.get(id.index())
    }

    pub(crate) fn op(&self, id: ValueId) -> Option<&Op> {
        self.value(id).map(|data| &data.op)
    }

    /// Whether the value has an effect beyond its result. A pure value can run for a lane that
    /// is not active, and a pass can delete it when nothing uses it.
    pub(crate) fn has_effect(&self, id: ValueId) -> bool {
        match self.op(id) {
            Some(Op::StoreVar { .. } | Op::MissTake | Op::MissOr(_) | Op::Return(_)) => true,
            Some(Op::If {
                then, otherwise, ..
            }) => self.region_has_effect(then) || self.region_has_effect(otherwise),
            _ => false,
        }
    }

    pub(crate) fn region_has_effect(&self, region: &Region) -> bool {
        region.insts.iter().any(|id| self.has_effect(*id))
    }

    pub(crate) fn constant(&self, id: ValueId) -> Option<f32> {
        match self.op(id) {
            Some(Op::Const(value)) => Some(*value),
            _ => None,
        }
    }
}

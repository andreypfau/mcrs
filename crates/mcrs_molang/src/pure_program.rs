use alloc::vec::Vec;

use crate::layout::VariableSlot;
use crate::math::{BinaryFn, TernaryFn, UnaryFn};
use crate::query::QuerySlot;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Register(pub(crate) u16);

impl Register {
    #[must_use]
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PureInput {
    This,
    Query(QuerySlot),
    Variable(VariableSlot),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PureInstr {
    Constant {
        dst: Register,
        value: f32,
    },
    Input {
        dst: Register,
        input: u16,
    },
    Unary {
        function: UnaryFn,
        dst: Register,
        a: Register,
    },
    Binary {
        function: BinaryFn,
        dst: Register,
        a: Register,
        b: Register,
    },
    Ternary {
        function: TernaryFn,
        dst: Register,
        a: Register,
        b: Register,
        c: Register,
    },
    Select {
        dst: Register,
        condition: Register,
        then: Register,
        otherwise: Register,
    },
    Move {
        dst: Register,
        src: Register,
    },
    If {
        condition: Register,
        otherwise: u32,
    },
    Else {
        end: u32,
    },
    EndIf,
    Return {
        src: Register,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct PureProgram {
    pub(crate) code: Vec<PureInstr>,
    pub(crate) registers: usize,
    pub(crate) inputs: Vec<PureInput>,
}

impl PureProgram {
    #[must_use]
    pub fn instructions(&self) -> &[PureInstr] {
        &self.code
    }

    #[must_use]
    pub const fn registers(&self) -> usize {
        self.registers
    }

    #[must_use]
    pub fn inputs(&self) -> &[PureInput] {
        &self.inputs
    }
}

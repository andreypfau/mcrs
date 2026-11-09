use alloc::vec::Vec;

use crate::layout::VariableSlot;
use crate::math::{BinaryFn, TernaryFn, UnaryFn};
use crate::query::QuerySlot;

/// A number register of a [`PureProgram`]. A register holds `0.0` before its first write.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Register(pub(crate) u16);

impl Register {
    #[must_use]
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

/// A value that a pure program reads from its subject. The host binds each input to a field of
/// the slot records or of the frame uniforms.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PureInput {
    /// The value of the channel before the instance applies it.
    This,
    Query(QuerySlot),
    /// A number variable; the CPU gives it as a scalar input.
    Variable(VariableSlot),
}

/// One instruction of a [`PureProgram`]. Control flow is structured: each `If` has one `Else`
/// and one `EndIf`.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PureInstr {
    Constant {
        dst: Register,
        value: f32,
    },
    /// Reads the input at this index of [`PureProgram::inputs`].
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
    /// `then` when `condition` is truthy, else `otherwise`; both are already computed.
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
    /// Runs the then-arm when `condition` is truthy; otherwise continues after the `Else` at the
    /// index `otherwise`.
    If {
        condition: Register,
        otherwise: u32,
    },
    /// Ends the then-arm, which continues after the `EndIf` at the index `end`.
    Else {
        end: u32,
    },
    EndIf,
    Return {
        src: Register,
    },
}

/// A program with one number result, no write, no string, no missing value and no effect: the
/// form of a channel that can run on a GPU. A run that reaches the end returns `0.0`.
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

    /// The number of registers.
    #[must_use]
    pub const fn registers(&self) -> usize {
        self.registers
    }

    /// The inputs in the order of first use.
    #[must_use]
    pub fn inputs(&self) -> &[PureInput] {
        &self.inputs
    }
}

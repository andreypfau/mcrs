//! A register bytecode that both VMs run. A register holds an `f32`, or a 64-bit value in the
//! second bank. Control flow is structured: `IfNot`, `Else` and `EndIf` nest, so the batch VM
//! can run both arms under lane masks and the scalar VM can jump.

use alloc::vec::Vec;

use crate::accesses::Accesses;
use crate::diagnostic::Span;
use crate::math::{BinaryFn, TernaryFn, UnaryFn};
use crate::value::{Rate, ValueType};

pub(crate) type Reg = u16;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Instr {
    LoadK {
        dst: Reg,
        value: f32,
    },
    LoadKBits {
        dst: Reg,
        value: u64,
    },
    This {
        dst: Reg,
    },
    Query {
        dst: Reg,
        slot: u16,
    },
    QueryBits {
        dst: Reg,
        slot: u16,
    },
    QueryMissing {
        dst: Reg,
        slot: u16,
    },
    LoadVar {
        dst: Reg,
        slot: u16,
    },
    LoadVarBits {
        dst: Reg,
        slot: u16,
    },
    VarMissing {
        dst: Reg,
        slot: u16,
    },
    VarMissingBits {
        dst: Reg,
        slot: u16,
    },
    StoreVar {
        slot: u16,
        src: Reg,
        missing: Reg,
    },
    StoreVarBits {
        slot: u16,
        src: Reg,
        missing: Reg,
    },
    MissTake {
        dst: Reg,
    },
    MissOr {
        src: Reg,
    },
    Unary {
        function: UnaryFn,
        dst: Reg,
        a: Reg,
    },
    Binary {
        function: BinaryFn,
        dst: Reg,
        a: Reg,
        b: Reg,
    },
    BinaryRk {
        function: BinaryFn,
        dst: Reg,
        a: Reg,
        k: f32,
    },
    BinaryKr {
        function: BinaryFn,
        dst: Reg,
        k: f32,
        b: Reg,
    },
    Ternary {
        function: TernaryFn,
        dst: Reg,
        a: Reg,
        b: Reg,
        c: Reg,
    },
    CompareBits {
        negate: bool,
        dst: Reg,
        a: Reg,
        b: Reg,
    },
    Select {
        dst: Reg,
        condition: Reg,
        a: Reg,
        b: Reg,
    },
    SelectBits {
        dst: Reg,
        condition: Reg,
        a: Reg,
        b: Reg,
    },
    Table {
        dst: Reg,
        table: u16,
        index: Reg,
    },
    TableBits {
        dst: Reg,
        table: u16,
        index: Reg,
    },
    Move {
        dst: Reg,
        src: Reg,
    },
    MoveBits {
        dst: Reg,
        src: Reg,
    },
    /// Starts the then-arm for the lanes where `condition` is truthy. The scalar VM jumps past
    /// the `Else` at `otherwise` when it is not.
    IfNot {
        condition: Reg,
        otherwise: u32,
    },
    /// Starts the else-arm. The scalar VM jumps to the `EndIf` at `end`.
    Else {
        condition: Reg,
        end: u32,
    },
    EndIf,
    Return {
        src: Reg,
    },
    ReturnBits {
        src: Reg,
    },
}

/// A compiled expression.
#[derive(Clone, Debug, PartialEq)]
pub struct Program {
    pub(crate) code: Vec<Instr>,
    pub(crate) spans: Vec<Span>,
    pub(crate) number_registers: usize,
    pub(crate) bits_registers: usize,
    pub(crate) tables: Vec<Vec<u64>>,
    pub(crate) output: ValueType,
    pub(crate) rate: Rate,
    pub(crate) accesses: Accesses,
}

impl Program {
    /// The type of the result.
    #[must_use]
    pub const fn output(&self) -> ValueType {
        self.output
    }

    /// The fastest rate of a value that the program reads. A program of the rate `Load` has one
    /// result for each subject.
    #[must_use]
    pub const fn rate(&self) -> Rate {
        self.rate
    }

    /// The number of instructions.
    #[must_use]
    pub fn len(&self) -> usize {
        self.code.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.code.is_empty()
    }
}

use alloc::vec::Vec;

use crate::bytecode::{Instr, Program};
use crate::compile::Expression;
use crate::layout::{Bank, VariableSlot};
use crate::query::QuerySlot;

/// The `variable.` slots and the query slots that a program reads and writes. Each list holds a
/// slot one time, in slot order. A host finds the inputs of a script here without a second
/// parse.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Accesses {
    /// The variables that the program reads, also only to test whether they are missing.
    pub variables_read: Vec<VariableSlot>,
    pub variables_written: Vec<VariableSlot>,
    /// The query and context slots that the program reads.
    pub queries_read: Vec<QuerySlot>,
}

impl Accesses {
    /// Whether the program reads a variable that it also writes, so that its result can depend
    /// on the number of runs.
    #[must_use]
    pub fn reads_own_write(&self) -> bool {
        self.variables_written
            .iter()
            .any(|slot| self.variables_read.binary_search(slot).is_ok())
    }

    /// Adds the slots of `other`.
    pub fn merge(&mut self, other: &Accesses) {
        self.variables_read.extend_from_slice(&other.variables_read);
        self.variables_written
            .extend_from_slice(&other.variables_written);
        self.queries_read.extend_from_slice(&other.queries_read);
        self.normalize();
    }

    fn normalize(&mut self) {
        for list in [&mut self.variables_read, &mut self.variables_written] {
            list.sort_unstable();
            list.dedup();
        }
        self.queries_read.sort_unstable();
        self.queries_read.dedup();
    }
}

fn variable(index: u16, bank: Bank) -> VariableSlot {
    VariableSlot { index, bank }
}

impl Accesses {
    pub(crate) fn of(code: &[Instr]) -> Self {
        let mut accesses = Self::default();
        for instr in code {
            match *instr {
                Instr::Query { slot, .. }
                | Instr::QueryBits { slot, .. }
                | Instr::QueryMissing { slot, .. } => accesses.queries_read.push(QuerySlot(slot)),
                Instr::LoadVar { slot, .. } | Instr::VarMissing { slot, .. } => {
                    accesses.variables_read.push(variable(slot, Bank::Number));
                }
                Instr::LoadVarBits { slot, .. } | Instr::VarMissingBits { slot, .. } => {
                    accesses.variables_read.push(variable(slot, Bank::Bits));
                }
                Instr::StoreVar { slot, .. } => {
                    accesses
                        .variables_written
                        .push(variable(slot, Bank::Number));
                }
                Instr::StoreVarBits { slot, .. } => {
                    accesses.variables_written.push(variable(slot, Bank::Bits));
                }
                _ => {}
            }
        }
        accesses.normalize();
        accesses
    }
}

static NO_ACCESSES: Accesses = Accesses {
    variables_read: Vec::new(),
    variables_written: Vec::new(),
    queries_read: Vec::new(),
};

impl Program {
    /// The variables and the query slots that the program reads and writes.
    #[must_use]
    pub fn accesses(&self) -> &Accesses {
        &self.accesses
    }
}

impl Expression {
    /// The variables and the query slots that the expression reads and writes. A constant has
    /// none.
    #[must_use]
    pub fn accesses(&self) -> &Accesses {
        match self {
            Self::Constant(_) => &NO_ACCESSES,
            Self::Program(program) => program.accesses(),
        }
    }
}

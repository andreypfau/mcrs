use alloc::vec::Vec;

use crate::bytecode::{Instr, Program, Reg};
use crate::math;
use crate::subject::Subject;
use crate::value::{Value, ValueType};

/// Runs a program for one subject. The VM keeps its registers between runs, so a run does not
/// allocate.
#[derive(Clone, Debug, Default)]
pub struct ScalarVm {
    numbers: Vec<f32>,
    bits: Vec<u64>,
}

impl ScalarVm {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Runs `program` once. Writes to `variable.` go to the store of the subject.
    pub fn run(&mut self, program: &Program, subject: &mut Subject<'_>) -> Value {
        if self.numbers.len() < program.number_registers {
            self.numbers.resize(program.number_registers, 0.0);
        }
        if self.bits.len() < program.bits_registers {
            self.bits.resize(program.bits_registers, 0);
        }
        let mut machine = Machine {
            numbers: &mut self.numbers,
            bits: &mut self.bits,
            subject,
            tables: &program.tables,
            missing: false,
        };
        machine.run(&program.code, program.output)
    }
}

struct Machine<'a, 'b> {
    numbers: &'a mut [f32],
    bits: &'a mut [u64],
    subject: &'a mut Subject<'b>,
    tables: &'a [Vec<u64>],
    missing: bool,
}

impl Machine<'_, '_> {
    #[inline]
    fn n(&self, register: Reg) -> f32 {
        self.numbers
            .get(usize::from(register))
            .copied()
            .unwrap_or(0.0)
    }

    #[inline]
    fn b(&self, register: Reg) -> u64 {
        self.bits.get(usize::from(register)).copied().unwrap_or(0)
    }

    #[inline]
    fn set_n(&mut self, register: Reg, value: f32) {
        if let Some(slot) = self.numbers.get_mut(usize::from(register)) {
            *slot = value;
        }
    }

    #[inline]
    fn set_b(&mut self, register: Reg, value: u64) {
        if let Some(slot) = self.bits.get_mut(usize::from(register)) {
            *slot = value;
        }
    }

    fn query_raw(&self, slot: u16) -> u64 {
        self.subject
            .queries
            .raw
            .get(usize::from(slot))
            .copied()
            .unwrap_or(0)
    }

    fn query_missing(&self, slot: u16) -> bool {
        !self
            .subject
            .queries
            .set
            .get(usize::from(slot))
            .copied()
            .unwrap_or(false)
    }

    fn table(&self, table: u16, index: f32) -> u64 {
        self.tables
            .get(usize::from(table))
            .and_then(|elements| math::table_element(elements, index))
            .unwrap_or(0)
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one flat match over the instruction set, one short arm for each instruction"
    )]
    fn run(&mut self, code: &[Instr], output: ValueType) -> Value {
        let mut pc = 0usize;
        while let Some(instr) = code.get(pc) {
            pc += 1;
            match *instr {
                Instr::LoadK { dst, value } => self.set_n(dst, value),
                Instr::LoadKBits { dst, value } => self.set_b(dst, value),
                Instr::This { dst } => self.set_n(dst, self.subject.this),
                Instr::Query { dst, slot } => {
                    self.set_n(dst, f32::from_bits(self.query_raw(slot) as u32));
                }
                Instr::QueryBits { dst, slot } => self.set_b(dst, self.query_raw(slot)),
                Instr::QueryMissing { dst, slot } => {
                    self.set_n(dst, math::number(self.query_missing(slot)));
                }
                Instr::LoadVar { dst, slot } => {
                    let value = self
                        .subject
                        .variables
                        .numbers
                        .get(usize::from(slot))
                        .copied()
                        .unwrap_or(0.0);
                    self.set_n(dst, value);
                }
                Instr::LoadVarBits { dst, slot } => {
                    let value = self
                        .subject
                        .variables
                        .bits
                        .get(usize::from(slot))
                        .copied()
                        .unwrap_or(0);
                    self.set_b(dst, value);
                }
                Instr::VarMissing { dst, slot } => {
                    let set = self
                        .subject
                        .variables
                        .number_set
                        .get(usize::from(slot))
                        .copied()
                        .unwrap_or(false);
                    self.set_n(dst, math::number(!set));
                }
                Instr::VarMissingBits { dst, slot } => {
                    let set = self
                        .subject
                        .variables
                        .bits_set
                        .get(usize::from(slot))
                        .copied()
                        .unwrap_or(false);
                    self.set_n(dst, math::number(!set));
                }
                Instr::StoreVar { slot, src, missing } => {
                    let missing = math::truthy(self.n(missing));
                    let value = if missing { 0.0 } else { self.n(src) };
                    let variables = &mut *self.subject.variables;
                    if let (Some(stored), Some(set)) = (
                        variables.numbers.get_mut(usize::from(slot)),
                        variables.number_set.get_mut(usize::from(slot)),
                    ) {
                        *stored = value;
                        *set = !missing;
                    }
                }
                Instr::StoreVarBits { slot, src, missing } => {
                    let missing = math::truthy(self.n(missing));
                    let value = if missing { 0 } else { self.b(src) };
                    let variables = &mut *self.subject.variables;
                    if let (Some(stored), Some(set)) = (
                        variables.bits.get_mut(usize::from(slot)),
                        variables.bits_set.get_mut(usize::from(slot)),
                    ) {
                        *stored = value;
                        *set = !missing;
                    }
                }
                Instr::MissTake { dst } => {
                    let flag = core::mem::replace(&mut self.missing, false);
                    self.set_n(dst, math::number(flag));
                }
                Instr::MissOr { src } => {
                    if math::truthy(self.n(src)) {
                        self.missing = true;
                    }
                }
                Instr::Unary { function, dst, a } => self.set_n(dst, function.apply(self.n(a))),
                Instr::Binary {
                    function,
                    dst,
                    a,
                    b,
                } => self.set_n(dst, function.apply(self.n(a), self.n(b))),
                Instr::BinaryRk {
                    function,
                    dst,
                    a,
                    k,
                } => self.set_n(dst, function.apply(self.n(a), k)),
                Instr::BinaryKr {
                    function,
                    dst,
                    k,
                    b,
                } => self.set_n(dst, function.apply(k, self.n(b))),
                Instr::Ternary {
                    function,
                    dst,
                    a,
                    b,
                    c,
                } => self.set_n(dst, function.apply(self.n(a), self.n(b), self.n(c))),
                Instr::CompareBits { negate, dst, a, b } => {
                    self.set_n(dst, math::number((self.b(a) == self.b(b)) != negate));
                }
                Instr::Select {
                    dst,
                    condition,
                    a,
                    b,
                } => {
                    let value = if math::truthy(self.n(condition)) {
                        self.n(a)
                    } else {
                        self.n(b)
                    };
                    self.set_n(dst, value);
                }
                Instr::SelectBits {
                    dst,
                    condition,
                    a,
                    b,
                } => {
                    let value = if math::truthy(self.n(condition)) {
                        self.b(a)
                    } else {
                        self.b(b)
                    };
                    self.set_b(dst, value);
                }
                Instr::Table { dst, table, index } => {
                    let raw = self.table(table, self.n(index));
                    self.set_n(dst, f32::from_bits(raw as u32));
                }
                Instr::TableBits { dst, table, index } => {
                    let raw = self.table(table, self.n(index));
                    self.set_b(dst, raw);
                }
                Instr::Move { dst, src } => self.set_n(dst, self.n(src)),
                Instr::MoveBits { dst, src } => self.set_b(dst, self.b(src)),
                Instr::IfNot {
                    condition,
                    otherwise,
                } => {
                    if !math::truthy(self.n(condition)) {
                        pc = otherwise as usize + 1;
                    }
                }
                Instr::Else { end, .. } => pc = end as usize + 1,
                Instr::EndIf => {}
                Instr::Return { src } => return Value::Number(self.n(src)),
                Instr::ReturnBits { src } => return Value::from_raw(output, self.b(src)),
            }
        }
        Value::Number(0.0)
    }
}

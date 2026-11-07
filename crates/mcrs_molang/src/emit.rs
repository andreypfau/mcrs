//! The emission of bytecode from the IR, with a linear scan over the structured regions to
//! give each value a register.

use alloc::vec;
use alloc::vec::Vec;

use crate::bytecode::{Instr, Program, Reg};
use crate::diagnostic::Span;
use crate::ir::{Function, Op, Region, ValueId};
use crate::layout::Bank;
use crate::value::{Rate, ValueType};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Form {
    Registers,
    RightConstant,
    LeftConstant,
}

fn binary_form(function: &Function, a: ValueId, b: ValueId) -> Form {
    if function.constant(b).is_some() {
        Form::RightConstant
    } else if function.constant(a).is_some() {
        Form::LeftConstant
    } else {
        Form::Registers
    }
}

/// The operands of an instruction that need a register.
fn register_uses(function: &Function, op: &Op) -> Vec<ValueId> {
    match *op {
        Op::Binary(_, a, b) => match binary_form(function, a, b) {
            Form::Registers => vec![a, b],
            Form::RightConstant => vec![a],
            Form::LeftConstant => vec![b],
        },
        Op::If { condition, .. } => vec![condition],
        _ => op.operands().as_slice().to_vec(),
    }
}

/// The positions of the uses of each value, in the order of emission.
struct Uses {
    last: Vec<Option<u32>>,
    position: u32,
}

impl Uses {
    fn record(&mut self, id: ValueId) {
        if let Some(slot) = self.last.get_mut(id.index()) {
            *slot = Some(self.position);
        }
    }

    fn walk(&mut self, function: &Function, region: &Region) {
        for id in &region.insts {
            let Some(op) = function.op(*id) else {
                continue;
            };
            if let Op::If {
                condition,
                then,
                otherwise,
                results,
            } = op
            {
                self.position += 1;
                self.record(*condition);
                self.arm(function, then, results);
                self.position += 1;
                self.record(*condition);
                self.arm(function, otherwise, results);
                self.position += 1;
                continue;
            }
            self.position += 1;
            for operand in register_uses(function, op) {
                self.record(operand);
            }
        }
    }

    fn arm(&mut self, function: &Function, region: &Region, results: &[ValueId]) {
        self.walk(function, region);
        self.position += 1;
        for (index, _) in results.iter().enumerate() {
            if let Some(value) = region.yields.get(index) {
                self.record(*value);
            }
        }
    }
}

struct Emitter<'a> {
    function: &'a Function,
    last: Vec<Option<u32>>,
    position: u32,
    registers: Vec<Option<Reg>>,
    free: [Vec<Reg>; 2],
    counts: [usize; 2],
    code: Vec<Instr>,
    spans: Vec<Span>,
}

const fn bank_index(bank: Bank) -> usize {
    match bank {
        Bank::Number => 0,
        Bank::Bits => 1,
    }
}

impl Emitter<'_> {
    fn bank_of(&self, id: ValueId) -> Bank {
        self.function
            .value(id)
            .map_or(Bank::Number, |data| Bank::from(data.value_type))
    }

    fn allocate(&mut self, id: ValueId) -> Reg {
        let bank = bank_index(self.bank_of(id));
        let register = match self.free.get_mut(bank).and_then(Vec::pop) {
            Some(register) => register,
            None => {
                let count = self.counts.get(bank).copied().unwrap_or(0);
                if let Some(slot) = self.counts.get_mut(bank) {
                    *slot += 1;
                }
                Reg::try_from(count).unwrap_or(Reg::MAX)
            }
        };
        if let Some(slot) = self.registers.get_mut(id.index()) {
            *slot = Some(register);
        }
        register
    }

    fn register(&self, id: ValueId) -> Option<Reg> {
        self.registers.get(id.index()).copied().flatten()
    }

    fn has_uses(&self, id: ValueId) -> bool {
        self.last.get(id.index()).copied().flatten().is_some()
    }

    /// Frees the register of `id` when this position is its last use.
    fn release(&mut self, id: ValueId) {
        if self.last.get(id.index()).copied().flatten() != Some(self.position) {
            return;
        }
        self.free_now(id);
    }

    fn free_now(&mut self, id: ValueId) {
        let bank = bank_index(self.bank_of(id));
        if let Some(register) = self.registers.get_mut(id.index()).and_then(Option::take)
            && let Some(free) = self.free.get_mut(bank)
        {
            free.push(register);
        }
    }

    fn push(&mut self, instr: Instr, span: Span) -> u32 {
        let index = u32::try_from(self.code.len()).unwrap_or(u32::MAX);
        self.code.push(instr);
        self.spans.push(span);
        index
    }

    fn reg(&self, id: ValueId) -> Reg {
        self.register(id).unwrap_or(0)
    }

    fn walk(&mut self, region: &Region) {
        let function = self.function;
        for id in &region.insts {
            let Some(data) = function.value(*id) else {
                continue;
            };
            if let Op::If {
                condition,
                then,
                otherwise,
                results,
            } = &data.op
            {
                self.position += 1;
                for result in results {
                    if self.has_uses(*result) {
                        self.allocate(*result);
                    }
                }
                let condition_register = self.reg(*condition);
                let if_index = self.push(
                    Instr::IfNot {
                        condition: condition_register,
                        otherwise: 0,
                    },
                    data.span,
                );
                self.arm(then, results, data.span);
                self.position += 1;
                let else_index = self.push(
                    Instr::Else {
                        condition: condition_register,
                        end: 0,
                    },
                    data.span,
                );
                self.release(*condition);
                if let Some(Instr::IfNot { otherwise, .. }) = self.code.get_mut(if_index as usize) {
                    *otherwise = else_index;
                }
                self.arm(otherwise, results, data.span);
                self.position += 1;
                let end_index = self.push(Instr::EndIf, data.span);
                if let Some(Instr::Else { end, .. }) = self.code.get_mut(else_index as usize) {
                    *end = end_index;
                }
                continue;
            }
            self.position += 1;
            let is_const = matches!(data.op, Op::Const(_) | Op::ConstBits(_));
            if is_const && !self.has_uses(*id) {
                continue;
            }
            let dst = match data.op {
                Op::StoreVar { .. } | Op::MissOr(_) | Op::Return(_) => 0,
                _ => self.allocate(*id),
            };
            let instr = self.instr(&data.op, dst, data.value_type);
            self.push(instr, data.span);
            for operand in register_uses(function, &data.op) {
                self.release(operand);
            }
            if !self.has_uses(*id) {
                self.free_now(*id);
            }
        }
    }

    fn arm(&mut self, region: &Region, results: &[ValueId], span: Span) {
        self.walk(region);
        self.position += 1;
        for (index, result) in results.iter().enumerate() {
            let Some(value) = region.yields.get(index) else {
                continue;
            };
            if let (Some(dst), Some(src)) = (self.register(*result), self.register(*value)) {
                let instr = match self.bank_of(*result) {
                    Bank::Number => Instr::Move { dst, src },
                    Bank::Bits => Instr::MoveBits { dst, src },
                };
                self.push(instr, span);
            }
        }
        for value in region.yields.iter().take(results.len()) {
            self.release(*value);
        }
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one flat match from operations to instructions, one short arm each"
    )]
    fn instr(&self, op: &Op, dst: Reg, value_type: ValueType) -> Instr {
        let bits = value_type.is_bits();
        match *op {
            Op::Const(value) => Instr::LoadK { dst, value },
            Op::ConstBits(value) => Instr::LoadKBits { dst, value },
            Op::This => Instr::This { dst },
            Op::Query(slot) if bits => Instr::QueryBits { dst, slot: slot.0 },
            Op::Query(slot) => Instr::Query { dst, slot: slot.0 },
            Op::QueryMissing(slot) => Instr::QueryMissing { dst, slot: slot.0 },
            Op::LoadVar(slot) => match slot.bank {
                Bank::Number => Instr::LoadVar {
                    dst,
                    slot: slot.index,
                },
                Bank::Bits => Instr::LoadVarBits {
                    dst,
                    slot: slot.index,
                },
            },
            Op::VarMissing(slot) => match slot.bank {
                Bank::Number => Instr::VarMissing {
                    dst,
                    slot: slot.index,
                },
                Bank::Bits => Instr::VarMissingBits {
                    dst,
                    slot: slot.index,
                },
            },
            Op::StoreVar {
                slot,
                value,
                missing,
            } => {
                let (src, missing) = (self.reg(value), self.reg(missing));
                match slot.bank {
                    Bank::Number => Instr::StoreVar {
                        slot: slot.index,
                        src,
                        missing,
                    },
                    Bank::Bits => Instr::StoreVarBits {
                        slot: slot.index,
                        src,
                        missing,
                    },
                }
            }
            Op::MissTake => Instr::MissTake { dst },
            Op::MissOr(flag) => Instr::MissOr {
                src: self.reg(flag),
            },
            Op::Unary(function, a) => Instr::Unary {
                function,
                dst,
                a: self.reg(a),
            },
            Op::Binary(function, a, b) => match binary_form(self.function, a, b) {
                Form::Registers => Instr::Binary {
                    function,
                    dst,
                    a: self.reg(a),
                    b: self.reg(b),
                },
                Form::RightConstant => Instr::BinaryRk {
                    function,
                    dst,
                    a: self.reg(a),
                    k: self.function.constant(b).unwrap_or(0.0),
                },
                Form::LeftConstant => Instr::BinaryKr {
                    function,
                    dst,
                    k: self.function.constant(a).unwrap_or(0.0),
                    b: self.reg(b),
                },
            },
            Op::Ternary(function, a, b, c) => Instr::Ternary {
                function,
                dst,
                a: self.reg(a),
                b: self.reg(b),
                c: self.reg(c),
            },
            Op::CompareBits { negate, a, b } => Instr::CompareBits {
                negate,
                dst,
                a: self.reg(a),
                b: self.reg(b),
            },
            Op::Select(condition, a, b) if bits => Instr::SelectBits {
                dst,
                condition: self.reg(condition),
                a: self.reg(a),
                b: self.reg(b),
            },
            Op::Select(condition, a, b) => Instr::Select {
                dst,
                condition: self.reg(condition),
                a: self.reg(a),
                b: self.reg(b),
            },
            Op::Table(table, index) if bits => Instr::TableBits {
                dst,
                table,
                index: self.reg(index),
            },
            Op::Table(table, index) => Instr::Table {
                dst,
                table,
                index: self.reg(index),
            },
            Op::Return(src) if bits => Instr::ReturnBits { src: self.reg(src) },
            Op::Return(src) => Instr::Return { src: self.reg(src) },
            Op::If { .. } | Op::Phi => Instr::EndIf,
        }
    }
}

/// The fastest rate of the values that the body reaches.
fn reached_rate(function: &Function, region: &Region) -> Rate {
    let mut rate = Rate::Constant;
    for id in &region.insts {
        let Some(data) = function.value(*id) else {
            continue;
        };
        rate = rate.max(data.rate);
        if let Op::If {
            then, otherwise, ..
        } = &data.op
        {
            rate = rate
                .max(reached_rate(function, then))
                .max(reached_rate(function, otherwise));
        }
    }
    rate
}

pub(crate) fn emit(function: &Function) -> Program {
    let mut uses = Uses {
        last: vec![None; function.values.len()],
        position: 0,
    };
    uses.walk(function, &function.body);
    let mut emitter = Emitter {
        function,
        last: uses.last,
        position: 0,
        registers: vec![None; function.values.len()],
        free: [Vec::new(), Vec::new()],
        counts: [0, 0],
        code: Vec::new(),
        spans: Vec::new(),
    };
    emitter.walk(&function.body);
    let [number_registers, bits_registers] = emitter.counts;
    Program {
        code: emitter.code,
        spans: emitter.spans,
        number_registers: number_registers.max(1),
        bits_registers: bits_registers.max(1),
        tables: function.tables.clone(),
        output: function.output,
        rate: reached_rate(function, &function.body),
    }
}

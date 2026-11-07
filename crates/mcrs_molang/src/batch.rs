use alloc::vec::Vec;

use crate::batch_subject::{BatchOutput, BatchSubjects};
use crate::bytecode::{Instr, Program, Reg};
use crate::math::{self, BinaryFn, UnaryFn};

/// Runs a program for a block of `N` lanes with the bytecode of the scalar VM. Each instruction
/// is one loop over the lanes. Pure instructions run for every lane; an instruction with an
/// effect runs only for the lanes of the current mask, so the results and the stores of each
/// lane are those of a scalar run.
#[derive(Clone, Debug)]
pub struct BatchVm<const N: usize> {
    numbers: Vec<[f32; N]>,
    bits: Vec<[u64; N]>,
    masks: Vec<[bool; N]>,
    mask: [bool; N],
    alive: [bool; N],
    missing: [bool; N],
    constant: [f32; N],
    out_numbers: [f32; N],
    out_bits: [u64; N],
}

impl<const N: usize> Default for BatchVm<N> {
    fn default() -> Self {
        Self::new()
    }
}

/// The register file with one register lent mutably and the others shared.
struct Split<'a, T, const N: usize> {
    before: &'a [[T; N]],
    after: &'a [[T; N]],
    at: usize,
}

impl<'a, T, const N: usize> Split<'a, T, N> {
    fn new(registers: &'a mut [[T; N]], at: Reg) -> Option<(&'a mut [T; N], Self)> {
        let at = usize::from(at);
        if at >= registers.len() {
            return None;
        }
        let (before, rest) = registers.split_at_mut(at);
        let (dst, after) = rest.split_first_mut()?;
        Some((dst, Self { before, after, at }))
    }

    fn get(&self, register: Reg) -> Option<&'a [T; N]> {
        let register = usize::from(register);
        if register < self.at {
            self.before.get(register)
        } else {
            self.after.get(register.checked_sub(self.at + 1)?)
        }
    }
}

#[inline(always)]
fn map1<const N: usize>(dst: &mut [f32; N], a: &[f32; N], function: impl Fn(f32) -> f32) {
    for (out, &x) in dst.iter_mut().zip(a) {
        *out = function(x);
    }
}

#[inline(always)]
fn map2<const N: usize>(
    dst: &mut [f32; N],
    a: &[f32; N],
    b: &[f32; N],
    function: impl Fn(f32, f32) -> f32,
) {
    for ((out, &x), &y) in dst.iter_mut().zip(a).zip(b) {
        *out = function(x, y);
    }
}

fn unary_column<const N: usize>(function: UnaryFn, dst: &mut [f32; N], a: &[f32; N]) {
    match function {
        UnaryFn::Negate => map1(dst, a, |x| -x),
        UnaryFn::Not => map1(dst, a, |x| UnaryFn::Not.apply(x)),
        UnaryFn::Truthy => map1(dst, a, |x| UnaryFn::Truthy.apply(x)),
        UnaryFn::Abs => map1(dst, a, |x| UnaryFn::Abs.apply(x)),
        UnaryFn::Floor => map1(dst, a, |x| UnaryFn::Floor.apply(x)),
        UnaryFn::Sign => map1(dst, a, |x| UnaryFn::Sign.apply(x)),
        other => map1(dst, a, |x| other.apply(x)),
    }
}

fn binary_column<const N: usize>(
    function: BinaryFn,
    dst: &mut [f32; N],
    a: &[f32; N],
    b: &[f32; N],
) {
    match function {
        BinaryFn::Add => map2(dst, a, b, |x, y| x + y),
        BinaryFn::Subtract => map2(dst, a, b, |x, y| x - y),
        BinaryFn::Multiply => map2(dst, a, b, |x, y| x * y),
        BinaryFn::Divide => map2(dst, a, b, math::divide),
        BinaryFn::Less => map2(dst, a, b, |x, y| math::number(x < y)),
        BinaryFn::LessEqual => map2(dst, a, b, |x, y| math::number(x <= y)),
        BinaryFn::Greater => map2(dst, a, b, |x, y| math::number(x > y)),
        BinaryFn::GreaterEqual => map2(dst, a, b, |x, y| math::number(x >= y)),
        BinaryFn::Equal => map2(dst, a, b, |x, y| math::number(x == y)),
        BinaryFn::NotEqual => map2(dst, a, b, |x, y| math::number(x != y)),
        BinaryFn::BothTruthy => map2(dst, a, b, |x, y| BinaryFn::BothTruthy.apply(x, y)),
        BinaryFn::EitherTruthy => map2(dst, a, b, |x, y| BinaryFn::EitherTruthy.apply(x, y)),
        BinaryFn::Min => map2(dst, a, b, math::min),
        BinaryFn::Max => map2(dst, a, b, math::max),
        other => map2(dst, a, b, |x, y| other.apply(x, y)),
    }
}

/// Stores `source` in the lanes of `mask`; a lane whose flag is truthy becomes missing.
fn store_masked<T: Copy, const N: usize>(
    stored: &mut [T; N],
    set: &mut [bool; N],
    source: &[T; N],
    flags: &[f32; N],
    mask: &[bool; N],
    zero: T,
) {
    let lanes = stored.iter_mut().zip(set.iter_mut()).zip(source).zip(flags);
    for ((((stored, set), &value), &flag), &active) in lanes.zip(mask) {
        if active {
            let is_missing = math::truthy(flag);
            *stored = if is_missing { zero } else { value };
            *set = !is_missing;
        }
    }
}

fn any<const N: usize>(mask: &[bool; N]) -> bool {
    mask.iter().any(|lane| *lane)
}

impl<const N: usize> BatchVm<N> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            numbers: Vec::new(),
            bits: Vec::new(),
            masks: Vec::new(),
            mask: [false; N],
            alive: [false; N],
            missing: [false; N],
            constant: [0.0; N],
            out_numbers: [0.0; N],
            out_bits: [0; N],
        }
    }

    /// Runs `program` once for each of the first `subjects.lanes` lanes.
    pub fn run(
        &mut self,
        program: &Program,
        subjects: &mut BatchSubjects<'_, N>,
    ) -> BatchOutput<'_, N> {
        let mut lanes = [false; N];
        for (lane, active) in lanes.iter_mut().enumerate() {
            *active = lane < subjects.lanes;
        }
        self.run_masked(program, subjects, &lanes)
    }

    /// Runs `program` once for each lane that `lanes` marks active; `subjects.lanes` is not
    /// read. An inactive lane keeps its variables, and its result is `0.0` or `0`.
    pub fn run_masked(
        &mut self,
        program: &Program,
        subjects: &mut BatchSubjects<'_, N>,
        lanes: &[bool; N],
    ) -> BatchOutput<'_, N> {
        if self.numbers.len() < program.number_registers {
            self.numbers.resize(program.number_registers, [0.0; N]);
        }
        if self.bits.len() < program.bits_registers {
            self.bits.resize(program.bits_registers, [0; N]);
        }
        self.mask = *lanes;
        self.alive = *lanes;
        self.missing = [false; N];
        self.out_numbers = [0.0; N];
        self.out_bits = [0; N];
        self.masks.clear();
        self.execute(program, subjects);
        if program.output.is_bits() {
            BatchOutput::Bits(&self.out_bits, program.output)
        } else {
            BatchOutput::Numbers(&self.out_numbers)
        }
    }

    fn number(&self, register: Reg) -> Option<&[f32; N]> {
        self.numbers.get(usize::from(register))
    }

    fn execute(&mut self, program: &Program, subjects: &mut BatchSubjects<'_, N>) {
        let mut pc = 0usize;
        while let Some(instr) = program.code.get(pc) {
            pc += 1;
            match *instr {
                Instr::IfNot {
                    condition,
                    otherwise,
                } => {
                    self.masks.push(self.mask);
                    if let Some(test) = self.numbers.get(usize::from(condition)) {
                        for (lane, &value) in self.mask.iter_mut().zip(test) {
                            *lane &= math::truthy(value);
                        }
                    }
                    if !any(&self.mask) {
                        pc = otherwise as usize;
                    }
                }
                Instr::Else { condition, end } => {
                    let parent = self.masks.last().copied().unwrap_or([false; N]);
                    if let Some(test) = self.numbers.get(usize::from(condition)) {
                        for (((lane, &value), &outer), &alive) in
                            self.mask.iter_mut().zip(test).zip(&parent).zip(&self.alive)
                        {
                            *lane = outer && alive && !math::truthy(value);
                        }
                    }
                    if !any(&self.mask) {
                        pc = end as usize;
                    }
                }
                Instr::EndIf => {
                    let parent = self.masks.pop().unwrap_or([false; N]);
                    for ((lane, outer), alive) in self.mask.iter_mut().zip(parent).zip(&self.alive)
                    {
                        *lane = outer && *alive;
                    }
                }
                _ => self.step(program, instr, subjects),
            }
        }
    }

    #[expect(
        clippy::too_many_lines,
        reason = "one flat match over the instruction set, one short arm for each instruction"
    )]
    fn step(&mut self, program: &Program, instr: &Instr, subjects: &mut BatchSubjects<'_, N>) {
        match *instr {
            Instr::LoadK { dst, value } => {
                if let Some(column) = self.numbers.get_mut(usize::from(dst)) {
                    *column = [value; N];
                }
            }
            Instr::LoadKBits { dst, value } => {
                if let Some(column) = self.bits.get_mut(usize::from(dst)) {
                    *column = [value; N];
                }
            }
            Instr::This { dst } => {
                if let Some(column) = self.numbers.get_mut(usize::from(dst)) {
                    *column = *subjects.this;
                }
            }
            Instr::Query { dst, slot } => {
                if let (Some(column), Some(source)) = (
                    self.numbers.get_mut(usize::from(dst)),
                    subjects.queries.numbers.get(usize::from(slot)),
                ) {
                    *column = *source;
                }
            }
            Instr::QueryBits { dst, slot } => {
                if let (Some(column), Some(source)) = (
                    self.bits.get_mut(usize::from(dst)),
                    subjects.queries.bits.get(usize::from(slot)),
                ) {
                    *column = *source;
                }
            }
            Instr::QueryMissing { dst, slot } => {
                let set = subjects.queries.set.get(usize::from(slot));
                Self::missing_column(self.numbers.get_mut(usize::from(dst)), set);
            }
            Instr::LoadVar { dst, slot } => {
                if let (Some(column), Some(source)) = (
                    self.numbers.get_mut(usize::from(dst)),
                    subjects.variables.numbers.get(usize::from(slot)),
                ) {
                    *column = *source;
                }
            }
            Instr::LoadVarBits { dst, slot } => {
                if let (Some(column), Some(source)) = (
                    self.bits.get_mut(usize::from(dst)),
                    subjects.variables.bits.get(usize::from(slot)),
                ) {
                    *column = *source;
                }
            }
            Instr::VarMissing { dst, slot } => {
                let set = subjects.variables.number_set.get(usize::from(slot));
                Self::missing_column(self.numbers.get_mut(usize::from(dst)), set);
            }
            Instr::VarMissingBits { dst, slot } => {
                let set = subjects.variables.bits_set.get(usize::from(slot));
                Self::missing_column(self.numbers.get_mut(usize::from(dst)), set);
            }
            Instr::StoreVar { slot, src, missing } => {
                let variables = &mut *subjects.variables;
                if let (Some(stored), Some(set), Some(source), Some(flags)) = (
                    variables.numbers.get_mut(usize::from(slot)),
                    variables.number_set.get_mut(usize::from(slot)),
                    self.numbers.get(usize::from(src)),
                    self.numbers.get(usize::from(missing)),
                ) {
                    store_masked(stored, set, source, flags, &self.mask, 0.0);
                }
            }
            Instr::StoreVarBits { slot, src, missing } => {
                let variables = &mut *subjects.variables;
                if let (Some(stored), Some(set), Some(source), Some(flags)) = (
                    variables.bits.get_mut(usize::from(slot)),
                    variables.bits_set.get_mut(usize::from(slot)),
                    self.bits.get(usize::from(src)),
                    self.numbers.get(usize::from(missing)),
                ) {
                    store_masked(stored, set, source, flags, &self.mask, 0);
                }
            }
            Instr::MissTake { dst } => {
                if let Some(column) = self.numbers.get_mut(usize::from(dst)) {
                    for ((out, flag), &active) in
                        column.iter_mut().zip(&mut self.missing).zip(&self.mask)
                    {
                        *out = math::number(*flag);
                        if active {
                            *flag = false;
                        }
                    }
                }
            }
            Instr::MissOr { src } => {
                if let Some(column) = self.numbers.get(usize::from(src)) {
                    for ((flag, &value), &active) in
                        self.missing.iter_mut().zip(column).zip(&self.mask)
                    {
                        *flag |= active && math::truthy(value);
                    }
                }
            }
            Instr::Unary { function, dst, a } => {
                if let Some((out, registers)) = Split::new(&mut self.numbers, dst)
                    && let Some(a) = registers.get(a)
                {
                    unary_column(function, out, a);
                }
            }
            Instr::Binary {
                function,
                dst,
                a,
                b,
            } => {
                if let Some((out, registers)) = Split::new(&mut self.numbers, dst)
                    && let (Some(a), Some(b)) = (registers.get(a), registers.get(b))
                {
                    binary_column(function, out, a, b);
                }
            }
            Instr::BinaryRk {
                function,
                dst,
                a,
                k,
            } => {
                self.constant = [k; N];
                if let Some((out, registers)) = Split::new(&mut self.numbers, dst)
                    && let Some(a) = registers.get(a)
                {
                    binary_column(function, out, a, &self.constant);
                }
            }
            Instr::BinaryKr {
                function,
                dst,
                k,
                b,
            } => {
                self.constant = [k; N];
                if let Some((out, registers)) = Split::new(&mut self.numbers, dst)
                    && let Some(b) = registers.get(b)
                {
                    binary_column(function, out, &self.constant, b);
                }
            }
            Instr::Ternary {
                function,
                dst,
                a,
                b,
                c,
            } => {
                if let Some((out, registers)) = Split::new(&mut self.numbers, dst)
                    && let (Some(a), Some(b), Some(c)) =
                        (registers.get(a), registers.get(b), registers.get(c))
                {
                    for (((out, &x), &y), &z) in out.iter_mut().zip(a).zip(b).zip(c) {
                        *out = function.apply(x, y, z);
                    }
                }
            }
            Instr::CompareBits { negate, dst, a, b } => {
                if let (Some(out), Some(a), Some(b)) = (
                    self.numbers.get_mut(usize::from(dst)),
                    self.bits.get(usize::from(a)),
                    self.bits.get(usize::from(b)),
                ) {
                    for ((out, &x), &y) in out.iter_mut().zip(a).zip(b) {
                        *out = math::number((x == y) != negate);
                    }
                }
            }
            Instr::Select {
                dst,
                condition,
                a,
                b,
            } => {
                if let Some((out, registers)) = Split::new(&mut self.numbers, dst)
                    && let (Some(test), Some(a), Some(b)) =
                        (registers.get(condition), registers.get(a), registers.get(b))
                {
                    for (((out, &test), &x), &y) in out.iter_mut().zip(test).zip(a).zip(b) {
                        *out = if math::truthy(test) { x } else { y };
                    }
                }
            }
            Instr::SelectBits {
                dst,
                condition,
                a,
                b,
            } => {
                let test = self.numbers.get(usize::from(condition));
                if let Some((out, registers)) = Split::new(&mut self.bits, dst)
                    && let (Some(test), Some(a), Some(b)) =
                        (test, registers.get(a), registers.get(b))
                {
                    for (((out, &test), &x), &y) in out.iter_mut().zip(test).zip(a).zip(b) {
                        *out = if math::truthy(test) { x } else { y };
                    }
                }
            }
            Instr::Table { dst, table, index } => {
                let elements = program.tables.get(usize::from(table));
                if let Some((out, registers)) = Split::new(&mut self.numbers, dst)
                    && let (Some(index), Some(elements)) = (registers.get(index), elements)
                {
                    for (out, &index) in out.iter_mut().zip(index) {
                        let raw = math::table_element(elements, index).unwrap_or(0);
                        *out = f32::from_bits(raw as u32);
                    }
                }
            }
            Instr::TableBits { dst, table, index } => {
                let elements = program.tables.get(usize::from(table));
                if let (Some(out), Some(index), Some(elements)) = (
                    self.bits.get_mut(usize::from(dst)),
                    self.numbers.get(usize::from(index)),
                    elements,
                ) {
                    for (out, &index) in out.iter_mut().zip(index) {
                        *out = math::table_element(elements, index).unwrap_or(0);
                    }
                }
            }
            Instr::Move { dst, src } => {
                if let Some((out, registers)) = Split::new(&mut self.numbers, dst)
                    && let Some(source) = registers.get(src)
                {
                    for ((out, &value), &active) in out.iter_mut().zip(source).zip(&self.mask) {
                        if active {
                            *out = value;
                        }
                    }
                }
            }
            Instr::MoveBits { dst, src } => {
                if let Some((out, registers)) = Split::new(&mut self.bits, dst)
                    && let Some(source) = registers.get(src)
                {
                    for ((out, &value), &active) in out.iter_mut().zip(source).zip(&self.mask) {
                        if active {
                            *out = value;
                        }
                    }
                }
            }
            Instr::Return { src } => {
                if let Some(source) = self.number(src) {
                    let source = *source;
                    self.finish(|lane| source.get(lane).copied().unwrap_or(0.0), None);
                }
            }
            Instr::ReturnBits { src } => {
                if let Some(source) = self.bits.get(usize::from(src)) {
                    let source = *source;
                    self.finish(|_| 0.0, Some(&source));
                }
            }
            Instr::IfNot { .. } | Instr::Else { .. } | Instr::EndIf => {}
        }
    }

    fn missing_column(column: Option<&mut [f32; N]>, set: Option<&[bool; N]>) {
        if let (Some(column), Some(set)) = (column, set) {
            for (out, &set) in column.iter_mut().zip(set) {
                *out = math::number(!set);
            }
        }
    }

    /// Ends the run of the active lanes with their results.
    fn finish(&mut self, number: impl Fn(usize) -> f32, bits: Option<&[u64; N]>) {
        for (lane, (((active, alive), out_number), out_bits)) in self
            .mask
            .iter_mut()
            .zip(&mut self.alive)
            .zip(&mut self.out_numbers)
            .zip(&mut self.out_bits)
            .enumerate()
        {
            if *active {
                match bits {
                    Some(bits) => *out_bits = bits.get(lane).copied().unwrap_or(0),
                    None => *out_number = number(lane),
                }
                *active = false;
                *alive = false;
            }
        }
    }
}

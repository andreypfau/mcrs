use alloc::vec;
use alloc::vec::Vec;

use crate::bytecode::{Instr, Program, Reg};
use crate::compile::Expression;
use crate::diagnostic::Span;
use crate::input_range::InputRange;
use crate::layout::{Bank, VariableSlot};
use crate::math::{BinaryFn, EaseDirection, EaseShape, TernaryFn, UnaryFn};
use crate::pure_program::{PureInput, PureInstr, PureProgram, Register};
use crate::query::QuerySlot;
use crate::value::Rate;

/// `exp` overflows above about `88.72`.
const EXP_LIMIT: f32 = 88.0;
/// `2^x` overflows at `128`.
const EXP2_LIMIT: f32 = 127.0;
/// The relative error that the range of a transcendental result allows for a GPU evaluation.
const INEXACT: f32 = 1e-5;
/// Above this magnitude, `min_angle` can leave `[-180, 180]` by more than a rounding step.
const ANGLE_LIMIT: f32 = 1e6;

/// The processor class of a channel expression.
#[derive(Clone, Debug, PartialEq)]
pub enum ChannelClass {
    /// The value is fixed for each pack load and host key: the host computes it once on the CPU.
    Constant,
    /// The expression can run on a GPU, as this program.
    Pure(PureProgram),
    /// The expression runs on the CPU, for this reason.
    State(StateReason),
}

/// Why an expression cannot run on a GPU.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StateReason {
    /// The expression reads or computes a string or a resource.
    String,
    /// The expression writes a variable.
    Write,
    /// The expression tests for a missing value, as `??` does.
    MissingValue,
    /// The expression selects from an array.
    Table,
    /// The expression reads a query that is not a GPU input.
    NotGpuInput(QuerySlot),
    /// A constant is NaN or infinite.
    NonFiniteConstant,
    /// The argument of a function can leave the domain where the result is finite.
    DomainRisk(Span),
    /// The program has more registers or inputs than a pure program can hold.
    TooLarge,
}

impl Expression {
    /// The processor class of the expression as a channel. `gpu_input` gives the range of each
    /// query that is a GPU input, and `None` for a query that is not one.
    pub fn classify(&self, gpu_input: impl FnMut(QuerySlot) -> Option<InputRange>) -> ChannelClass {
        match self {
            Self::Constant(_) => ChannelClass::Constant,
            Self::Program(program) => program.classify(gpu_input),
        }
    }
}

impl Program {
    /// The processor class of the program as a channel; see [`Expression::classify`].
    pub fn classify(&self, gpu_input: impl FnMut(QuerySlot) -> Option<InputRange>) -> ChannelClass {
        if self.rate <= Rate::Load {
            return ChannelClass::Constant;
        }
        let mut converter = Converter {
            gpu_input,
            code: Vec::with_capacity(self.code.len()),
            inputs: Vec::new(),
            ranges: vec![InputRange::EMPTY; self.number_registers],
            frames: Vec::new(),
        };
        for (index, instr) in self.code.iter().enumerate() {
            let span = self.spans.get(index).copied().unwrap_or_default();
            if let Err(reason) = converter.instr(*instr, span) {
                return ChannelClass::State(reason);
            }
        }
        ChannelClass::Pure(PureProgram {
            code: converter.code,
            registers: converter.ranges.len(),
            inputs: converter.inputs,
        })
    }
}

enum Frame {
    Then {
        if_at: usize,
        before: Vec<InputRange>,
    },
    Otherwise {
        else_at: usize,
        then: Vec<InputRange>,
    },
}

struct Converter<F> {
    gpu_input: F,
    code: Vec<PureInstr>,
    inputs: Vec<PureInput>,
    ranges: Vec<InputRange>,
    frames: Vec<Frame>,
}

impl<F: FnMut(QuerySlot) -> Option<InputRange>> Converter<F> {
    fn range(&self, register: Reg) -> InputRange {
        self.ranges
            .get(usize::from(register))
            .copied()
            .unwrap_or(InputRange::ANY)
    }

    fn define(&mut self, dst: Reg, range: InputRange) {
        if let Some(slot) = self.ranges.get_mut(usize::from(dst)) {
            *slot = range;
        }
    }

    fn input(&mut self, dst: Reg, input: PureInput, range: InputRange) -> Result<(), StateReason> {
        let index = match self.inputs.iter().position(|known| *known == input) {
            Some(index) => index,
            None => {
                self.inputs.push(input);
                self.inputs.len() - 1
            }
        };
        let input = u16::try_from(index).map_err(|_| StateReason::TooLarge)?;
        self.code.push(PureInstr::Input {
            dst: Register(dst),
            input,
        });
        self.define(dst, range);
        Ok(())
    }

    /// A register that holds the constant `value`, for the instructions with a constant operand.
    fn constant_register(&mut self, value: f32) -> Result<Reg, StateReason> {
        if !value.is_finite() {
            return Err(StateReason::NonFiniteConstant);
        }
        let register = u16::try_from(self.ranges.len()).map_err(|_| StateReason::TooLarge)?;
        self.ranges.push(InputRange::point(value));
        self.code.push(PureInstr::Constant {
            dst: Register(register),
            value,
        });
        Ok(register)
    }

    fn instr(&mut self, instr: Instr, span: Span) -> Result<(), StateReason> {
        match instr {
            Instr::LoadK { dst, value } => {
                if !value.is_finite() {
                    return Err(StateReason::NonFiniteConstant);
                }
                self.code.push(PureInstr::Constant {
                    dst: Register(dst),
                    value,
                });
                self.define(dst, InputRange::point(value));
            }
            Instr::This { dst } => self.input(dst, PureInput::This, InputRange::FINITE)?,
            Instr::Query { dst, slot } => {
                let slot = QuerySlot(slot);
                let range = (self.gpu_input)(slot).ok_or(StateReason::NotGpuInput(slot))?;
                self.input(dst, PureInput::Query(slot), range)?;
            }
            Instr::LoadVar { dst, slot } => {
                let slot = VariableSlot {
                    index: slot,
                    bank: Bank::Number,
                };
                self.input(dst, PureInput::Variable(slot), InputRange::FINITE)?;
            }
            Instr::Unary { function, dst, a } => self.unary(function, dst, a, span)?,
            Instr::Binary {
                function,
                dst,
                a,
                b,
            } => self.binary(function, dst, a, b, span)?,
            Instr::BinaryRk {
                function,
                dst,
                a,
                k,
            } => {
                let b = self.constant_register(k)?;
                self.binary(function, dst, a, b, span)?;
            }
            Instr::BinaryKr {
                function,
                dst,
                k,
                b,
            } => {
                let a = self.constant_register(k)?;
                self.binary(function, dst, a, b, span)?;
            }
            Instr::Ternary {
                function,
                dst,
                a,
                b,
                c,
            } => self.ternary(function, dst, [a, b, c], span)?,
            Instr::Select {
                dst,
                condition,
                a,
                b,
            } => {
                self.code.push(PureInstr::Select {
                    dst: Register(dst),
                    condition: Register(condition),
                    then: Register(a),
                    otherwise: Register(b),
                });
                self.define(dst, self.range(a).union(self.range(b)));
            }
            Instr::Move { dst, src } => {
                self.code.push(PureInstr::Move {
                    dst: Register(dst),
                    src: Register(src),
                });
                self.define(dst, self.range(src));
            }
            Instr::IfNot { condition, .. } => self.open(condition),
            Instr::Else { .. } => self.otherwise(),
            Instr::EndIf => self.close(),
            Instr::Return { src } => self.code.push(PureInstr::Return { src: Register(src) }),
            Instr::StoreVar { .. } => return Err(StateReason::Write),
            Instr::QueryMissing { .. }
            | Instr::VarMissing { .. }
            | Instr::MissTake { .. }
            | Instr::MissOr { .. } => return Err(StateReason::MissingValue),
            Instr::Table { .. } => return Err(StateReason::Table),
            Instr::LoadKBits { .. }
            | Instr::QueryBits { .. }
            | Instr::LoadVarBits { .. }
            | Instr::VarMissingBits { .. }
            | Instr::StoreVarBits { .. }
            | Instr::CompareBits { .. }
            | Instr::SelectBits { .. }
            | Instr::TableBits { .. }
            | Instr::MoveBits { .. }
            | Instr::ReturnBits { .. } => return Err(StateReason::String),
        }
        Ok(())
    }

    fn open(&mut self, condition: Reg) {
        self.frames.push(Frame::Then {
            if_at: self.code.len(),
            before: self.ranges.clone(),
        });
        self.code.push(PureInstr::If {
            condition: Register(condition),
            otherwise: 0,
        });
    }

    fn otherwise(&mut self) {
        let else_at = self.code.len();
        self.code.push(PureInstr::Else { end: 0 });
        if let Some(Frame::Then { if_at, mut before }) = self.frames.pop() {
            if let Some(PureInstr::If { otherwise, .. }) = self.code.get_mut(if_at) {
                *otherwise = u32::try_from(else_at).unwrap_or(u32::MAX);
            }
            before.resize(self.ranges.len(), InputRange::EMPTY);
            let then = core::mem::replace(&mut self.ranges, before);
            self.frames.push(Frame::Otherwise { else_at, then });
        }
    }

    fn close(&mut self) {
        let end_at = self.code.len();
        self.code.push(PureInstr::EndIf);
        if let Some(Frame::Otherwise { else_at, mut then }) = self.frames.pop() {
            if let Some(PureInstr::Else { end }) = self.code.get_mut(else_at) {
                *end = u32::try_from(end_at).unwrap_or(u32::MAX);
            }
            then.resize(self.ranges.len(), InputRange::EMPTY);
            for (range, other) in self.ranges.iter_mut().zip(then) {
                *range = range.union(other);
            }
        }
    }

    fn unary(
        &mut self,
        function: UnaryFn,
        dst: Reg,
        a: Reg,
        span: Span,
    ) -> Result<(), StateReason> {
        let x = self.range(a);
        let in_domain = match function {
            UnaryFn::Sqrt => x.min >= 0.0,
            UnaryFn::Ln => x.min > 0.0,
            UnaryFn::Exp => x.max <= EXP_LIMIT,
            UnaryFn::Asin | UnaryFn::Acos => x.min >= -1.0 && x.max <= 1.0,
            _ => true,
        };
        if !in_domain {
            return Err(StateReason::DomainRisk(span));
        }
        self.code.push(PureInstr::Unary {
            function,
            dst: Register(dst),
            a: Register(a),
        });
        self.define(dst, unary_range(function, x));
        Ok(())
    }

    fn binary(
        &mut self,
        function: BinaryFn,
        dst: Reg,
        a: Reg,
        b: Reg,
        span: Span,
    ) -> Result<(), StateReason> {
        let (x, y) = (self.range(a), self.range(b));
        if function == BinaryFn::Pow && !pow_in_domain(x, y) {
            return Err(StateReason::DomainRisk(span));
        }
        self.code.push(PureInstr::Binary {
            function,
            dst: Register(dst),
            a: Register(a),
            b: Register(b),
        });
        self.define(dst, binary_range(function, x, y));
        Ok(())
    }

    fn ternary(
        &mut self,
        function: TernaryFn,
        dst: Reg,
        [a, b, c]: [Reg; 3],
        span: Span,
    ) -> Result<(), StateReason> {
        let (x, y, z) = (self.range(a), self.range(b), self.range(c));
        if let TernaryFn::Ease(easing) = function
            && !ease_in_domain(easing.direction, easing.shape, z)
        {
            return Err(StateReason::DomainRisk(span));
        }
        self.code.push(PureInstr::Ternary {
            function,
            dst: Register(dst),
            a: Register(a),
            b: Register(b),
            c: Register(c),
        });
        self.define(dst, ternary_range(function, x, y, z));
        Ok(())
    }
}

fn inexact(range: InputRange) -> InputRange {
    InputRange::new(
        range.min - range.min.abs() * INEXACT,
        range.max + range.max.abs() * INEXACT,
    )
}

fn angle(range: InputRange) -> InputRange {
    if range.largest_magnitude() <= ANGLE_LIMIT {
        InputRange::new(-181.0, 181.0)
    } else {
        InputRange::ANY
    }
}

fn unary_range(function: UnaryFn, x: InputRange) -> InputRange {
    match function {
        UnaryFn::Negate => x.negate(),
        UnaryFn::Not | UnaryFn::Truthy => InputRange::BOOLEAN,
        UnaryFn::Abs => x.abs(),
        UnaryFn::Ceil => x.monotone(libm::ceilf),
        UnaryFn::Floor => x.monotone(libm::floorf),
        UnaryFn::Trunc => x.monotone(libm::truncf),
        UnaryFn::Round => x.monotone(libm::roundf),
        UnaryFn::Sqrt => x.monotone(libm::sqrtf),
        UnaryFn::Exp => {
            let range = inexact(x.monotone(libm::expf));
            InputRange::new(range.min.max(0.0), range.max)
        }
        UnaryFn::Ln => inexact(x.monotone(libm::logf)),
        UnaryFn::Sin | UnaryFn::Cos | UnaryFn::Sign => InputRange::new(-1.0, 1.0),
        UnaryFn::Asin | UnaryFn::Atan => inexact(InputRange::new(-90.0, 90.0)),
        UnaryFn::Acos => inexact(InputRange::new(0.0, 180.0)),
        UnaryFn::MinAngle => angle(x),
        UnaryFn::HermiteBlend => {
            let square = x.multiply(x);
            InputRange::point(3.0)
                .multiply(square)
                .subtract(InputRange::point(2.0).multiply(square.multiply(x)))
        }
    }
}

fn binary_range(function: BinaryFn, x: InputRange, y: InputRange) -> InputRange {
    match function {
        BinaryFn::Add => x.add(y),
        BinaryFn::Subtract => x.subtract(y),
        BinaryFn::Multiply => x.multiply(y),
        BinaryFn::Divide => x.divide(y),
        BinaryFn::Less
        | BinaryFn::LessEqual
        | BinaryFn::Greater
        | BinaryFn::GreaterEqual
        | BinaryFn::Equal
        | BinaryFn::NotEqual
        | BinaryFn::BothTruthy
        | BinaryFn::EitherTruthy
        | BinaryFn::ApproxEqual => InputRange::BOOLEAN,
        BinaryFn::Min => x.min_with(y),
        BinaryFn::Max => x.max_with(y),
        BinaryFn::Mod => x.remainder(y),
        BinaryFn::Pow => InputRange::new(0.0, pow_bound(x, y)),
        BinaryFn::Atan2 => inexact(InputRange::new(-180.0, 180.0)),
        BinaryFn::CopySign => x.any_sign(),
    }
}

fn ternary_range(function: TernaryFn, x: InputRange, y: InputRange, z: InputRange) -> InputRange {
    match function {
        TernaryFn::Clamp => x.max_with(y).min_with(z),
        TernaryFn::Lerp => x.add(y.subtract(x).multiply(z)),
        TernaryFn::InverseLerp => z.subtract(x).divide(y.subtract(x)),
        TernaryFn::LerpRotate => angle(x.union(y).union(z)),
        TernaryFn::InRange => InputRange::BOOLEAN,
        TernaryFn::Ease(_) => InputRange::ANY,
    }
}

/// The largest value of `e · ln(b)` over the corners, in `f64`, or `None` when a bound is not
/// finite or the base can be `0` or less.
fn pow_exponent(base: InputRange, exponent: InputRange) -> Option<f64> {
    let finite = [base.min, base.max, exponent.min, exponent.max]
        .iter()
        .all(|bound| bound.is_finite());
    if !finite || base.min <= 0.0 {
        return None;
    }
    let logs = [
        libm::log(f64::from(base.min)),
        libm::log(f64::from(base.max)),
    ];
    let exponents = [f64::from(exponent.min), f64::from(exponent.max)];
    let mut largest = f64::NEG_INFINITY;
    for log in logs {
        for exponent in exponents {
            largest = largest.max(log * exponent);
        }
    }
    Some(largest)
}

fn pow_in_domain(base: InputRange, exponent: InputRange) -> bool {
    pow_exponent(base, exponent).is_some_and(|largest| largest <= f64::from(EXP_LIMIT))
}

fn pow_bound(base: InputRange, exponent: InputRange) -> f32 {
    pow_exponent(base, exponent).map_or(f32::INFINITY, |largest| {
        (libm::exp(largest) * (1.0 + f64::from(INEXACT))) as f32
    })
}

/// Whether the inner function of an easing stays where its result is finite. The inner argument
/// is `t`, `1 − t`, `2t` or `2 − 2t`, by the direction.
fn ease_in_domain(direction: EaseDirection, shape: EaseShape, t: InputRange) -> bool {
    let one = InputRange::point(1.0);
    let two = InputRange::point(2.0);
    let inner = match direction {
        EaseDirection::In => t,
        EaseDirection::Out => one.subtract(t),
        EaseDirection::InOut => two.multiply(t).union(two.subtract(two.multiply(t))),
    };
    match shape {
        EaseShape::Circ => inner.min >= -1.0 && inner.max <= 1.0,
        EaseShape::Expo | EaseShape::Elastic => {
            let power = InputRange::point(10.0)
                .multiply(inner)
                .subtract(InputRange::point(10.0));
            power.max <= EXP2_LIMIT
        }
        _ => true,
    }
}

//! The one implementation of each operator and function. Both VMs and the constant folder call
//! these functions, so a folded value and a run value have the same bits.

use crate::value::Milestone;

const DEGREES_TO_RADIANS: f32 = core::f32::consts::PI / 180.0;
const RADIANS_TO_DEGREES: f32 = 180.0 / core::f32::consts::PI;
const APPROX_EQ_TOLERANCE: f32 = 0.000_000_1;

/// The element that an index selects: NaN and the infinities select the first element, and
/// each other index is truncated, limited below by `0` and wrapped by the length.
#[must_use]
pub(crate) fn table_element(elements: &[u64], index: f32) -> Option<u64> {
    let position = if index.is_finite() {
        let truncated = libm::truncf(index).max(0.0);
        // `as` saturates, and the remainder keeps the position inside the table.
        (truncated as u64 % elements.len().max(1) as u64) as usize
    } else {
        0
    };
    elements.get(position).copied()
}

#[inline]
#[must_use]
pub fn truthy(value: f32) -> bool {
    value != 0.0
}

#[inline]
#[must_use]
pub fn number(condition: bool) -> f32 {
    if condition { 1.0 } else { 0.0 }
}

#[inline]
#[must_use]
pub fn divide(dividend: f32, divisor: f32) -> f32 {
    if divisor == 0.0 {
        0.0
    } else {
        dividend / divisor
    }
}

/// The smaller value; a NaN operand gives the other operand.
#[inline]
#[must_use]
pub fn min(a: f32, b: f32) -> f32 {
    if b < a || a.is_nan() { b } else { a }
}

/// The larger value; a NaN operand gives the other operand.
#[inline]
#[must_use]
pub fn max(a: f32, b: f32) -> f32 {
    if b > a || a.is_nan() { b } else { a }
}

fn reduce_degrees(degrees: f32) -> f32 {
    let reduced = libm::fmodf(degrees, 360.0);
    if reduced > 180.0 {
        reduced - 360.0
    } else if reduced < -180.0 {
        reduced + 360.0
    } else {
        reduced
    }
}

#[must_use]
pub fn sin_degrees(degrees: f32) -> f32 {
    libm::sinf(reduce_degrees(degrees) * DEGREES_TO_RADIANS)
}

#[must_use]
pub fn cos_degrees(degrees: f32) -> f32 {
    libm::cosf(reduce_degrees(degrees) * DEGREES_TO_RADIANS)
}

fn floor_mod(a: f32, b: f32) -> f32 {
    if b == 0.0 { 0.0 } else { libm::fmodf(a, b) }
}

fn min_angle(angle: f32) -> f32 {
    angle - (360.0 * libm::floorf((angle + 180.0) / 360.0))
}

fn lerp(start: f32, end: f32, t: f32) -> f32 {
    start + ((end - start) * t)
}

/// A function of one number.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UnaryFn {
    Negate,
    Not,
    Truthy,
    Abs,
    Ceil,
    Floor,
    Trunc,
    Round,
    Sqrt,
    Exp,
    Ln,
    Sin,
    Cos,
    Asin,
    Acos,
    Atan,
    Sign,
    MinAngle,
    HermiteBlend,
}

impl UnaryFn {
    #[must_use]
    pub fn apply(self, x: f32) -> f32 {
        match self {
            Self::Negate => -x,
            Self::Not => number(!truthy(x)),
            Self::Truthy => number(truthy(x)),
            Self::Abs => libm::fabsf(x),
            Self::Ceil => libm::ceilf(x),
            Self::Floor => libm::floorf(x),
            Self::Trunc => libm::truncf(x),
            Self::Round => libm::roundf(x),
            Self::Sqrt => libm::sqrtf(x),
            Self::Exp => libm::expf(x),
            Self::Ln => libm::logf(x),
            Self::Sin => sin_degrees(x),
            Self::Cos => cos_degrees(x),
            Self::Asin => libm::asinf(x) * RADIANS_TO_DEGREES,
            Self::Acos => libm::acosf(x) * RADIANS_TO_DEGREES,
            Self::Atan => libm::atanf(x) * RADIANS_TO_DEGREES,
            Self::Sign => {
                if x > 0.0 {
                    1.0
                } else {
                    -1.0
                }
            }
            Self::MinAngle => min_angle(x),
            Self::HermiteBlend => (3.0 * (x * x)) - (2.0 * ((x * x) * x)),
        }
    }
}

/// A function of two numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BinaryFn {
    Add,
    Subtract,
    Multiply,
    Divide,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Equal,
    NotEqual,
    /// `1.0` when both operands are truthy, with no laziness.
    BothTruthy,
    /// `1.0` when one operand is truthy, with no laziness.
    EitherTruthy,
    ApproxEqual,
    Min,
    Max,
    Mod,
    Pow,
    Atan2,
    CopySign,
}

impl BinaryFn {
    #[must_use]
    pub fn apply(self, a: f32, b: f32) -> f32 {
        match self {
            Self::Add => a + b,
            Self::Subtract => a - b,
            Self::Multiply => a * b,
            Self::Divide => divide(a, b),
            Self::Less => number(a < b),
            Self::LessEqual => number(a <= b),
            Self::Greater => number(a > b),
            Self::GreaterEqual => number(a >= b),
            Self::Equal => number(a == b),
            Self::NotEqual => number(a != b),
            Self::BothTruthy => number(truthy(a) && truthy(b)),
            Self::EitherTruthy => number(truthy(a) || truthy(b)),
            Self::ApproxEqual => number(libm::fabsf(a - b) <= APPROX_EQ_TOLERANCE),
            Self::Min => min(a, b),
            Self::Max => max(a, b),
            Self::Mod => floor_mod(a, b),
            Self::Pow => libm::powf(a, b),
            Self::Atan2 => libm::atan2f(a, b) * RADIANS_TO_DEGREES,
            Self::CopySign => libm::copysignf(a, b),
        }
    }

    /// Whether the operation commutes with exactly equal bits, so that CSE can sort operands.
    #[must_use]
    pub const fn commutes(self) -> bool {
        matches!(
            self,
            Self::Add
                | Self::Multiply
                | Self::Equal
                | Self::NotEqual
                | Self::BothTruthy
                | Self::EitherTruthy
        )
    }
}

/// The direction of an easing function.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EaseDirection {
    In,
    Out,
    InOut,
}

/// The shape of an easing function.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EaseShape {
    Quad,
    Cubic,
    Quart,
    Quint,
    Sine,
    Expo,
    Circ,
    Back,
    Elastic,
    Bounce,
}

impl EaseShape {
    fn ease_in(self, t: f32) -> f32 {
        match self {
            Self::Quad => t * t,
            Self::Cubic => t * t * t,
            Self::Quart => t * t * t * t,
            Self::Quint => t * t * t * t * t,
            Self::Sine => 1.0 - cos_degrees(90.0 * t),
            Self::Expo => {
                if t == 0.0 {
                    0.0
                } else {
                    libm::powf(2.0, (10.0 * t) - 10.0)
                }
            }
            Self::Circ => 1.0 - libm::sqrtf(1.0 - (t * t)),
            Self::Back => (2.701_58 * (t * t * t)) - (1.701_58 * (t * t)),
            Self::Elastic => {
                if t == 0.0 {
                    0.0
                } else if t == 1.0 {
                    1.0
                } else {
                    -libm::powf(2.0, (10.0 * t) - 10.0) * sin_degrees(((10.0 * t) - 10.75) * 120.0)
                }
            }
            Self::Bounce => 1.0 - bounce(1.0 - t),
        }
    }
}

fn bounce(u: f32) -> f32 {
    const N: f32 = 7.5625;
    const D: f32 = 2.75;
    if u < 1.0 / D {
        N * (u * u)
    } else if u < 2.0 / D {
        let v = u - (1.5 / D);
        (N * (v * v)) + 0.75
    } else if u < 2.5 / D {
        let v = u - (2.25 / D);
        (N * (v * v)) + 0.9375
    } else {
        let v = u - (2.625 / D);
        (N * (v * v)) + 0.984_375
    }
}

/// An easing function `math.ease_<direction>_<shape>`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Easing {
    pub direction: EaseDirection,
    pub shape: EaseShape,
}

impl Easing {
    fn progress(self, t: f32) -> f32 {
        match self.direction {
            EaseDirection::In => self.shape.ease_in(t),
            EaseDirection::Out => 1.0 - self.shape.ease_in(1.0 - t),
            EaseDirection::InOut => {
                if t < 0.5 {
                    self.shape.ease_in(2.0 * t) / 2.0
                } else {
                    1.0 - (self.shape.ease_in(2.0 - (2.0 * t)) / 2.0)
                }
            }
        }
    }

    fn parse(name: &str) -> Option<Self> {
        let rest = name.strip_prefix("ease_")?;
        let (direction, shape) = if let Some(shape) = rest.strip_prefix("in_out_") {
            (EaseDirection::InOut, shape)
        } else if let Some(shape) = rest.strip_prefix("in_") {
            (EaseDirection::In, shape)
        } else {
            (EaseDirection::Out, rest.strip_prefix("out_")?)
        };
        let shape = match shape {
            "quad" => EaseShape::Quad,
            "cubic" => EaseShape::Cubic,
            "quart" => EaseShape::Quart,
            "quint" => EaseShape::Quint,
            "sine" => EaseShape::Sine,
            "expo" => EaseShape::Expo,
            "circ" => EaseShape::Circ,
            "back" => EaseShape::Back,
            "elastic" => EaseShape::Elastic,
            "bounce" => EaseShape::Bounce,
            _ => return None,
        };
        Some(Self { direction, shape })
    }
}

/// A function of three numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TernaryFn {
    Clamp,
    Lerp,
    InverseLerp,
    LerpRotate,
    InRange,
    Ease(Easing),
}

impl TernaryFn {
    #[must_use]
    pub fn apply(self, a: f32, b: f32, c: f32) -> f32 {
        match self {
            Self::Clamp => min(max(a, b), c),
            Self::Lerp => lerp(a, b, c),
            Self::InverseLerp => divide(c - a, b - a),
            Self::LerpRotate => min_angle(a + (min_angle(b - a) * c)),
            Self::InRange => number(b <= a && a <= c),
            Self::Ease(easing) => lerp(a, b, easing.progress(c)),
        }
    }
}

/// A `math.` name after the lookup.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum MathName {
    Pi,
    Unary(UnaryFn),
    Binary(BinaryFn),
    Ternary(TernaryFn),
    /// A random function; the compiler does not implement random streams.
    Random(u8),
}

impl MathName {
    pub(crate) fn lookup(name: &str) -> Option<Self> {
        let found = match name {
            "pi" => Self::Pi,
            "abs" => Self::Unary(UnaryFn::Abs),
            "ceil" => Self::Unary(UnaryFn::Ceil),
            "floor" => Self::Unary(UnaryFn::Floor),
            "trunc" => Self::Unary(UnaryFn::Trunc),
            "round" => Self::Unary(UnaryFn::Round),
            "sqrt" => Self::Unary(UnaryFn::Sqrt),
            "exp" => Self::Unary(UnaryFn::Exp),
            "ln" => Self::Unary(UnaryFn::Ln),
            "sin" => Self::Unary(UnaryFn::Sin),
            "cos" => Self::Unary(UnaryFn::Cos),
            "asin" => Self::Unary(UnaryFn::Asin),
            "acos" => Self::Unary(UnaryFn::Acos),
            "atan" => Self::Unary(UnaryFn::Atan),
            "sign" => Self::Unary(UnaryFn::Sign),
            "min_angle" => Self::Unary(UnaryFn::MinAngle),
            "hermite_blend" => Self::Unary(UnaryFn::HermiteBlend),
            "pow" => Self::Binary(BinaryFn::Pow),
            "atan2" => Self::Binary(BinaryFn::Atan2),
            "min" => Self::Binary(BinaryFn::Min),
            "max" => Self::Binary(BinaryFn::Max),
            "mod" => Self::Binary(BinaryFn::Mod),
            "copy_sign" => Self::Binary(BinaryFn::CopySign),
            "clamp" => Self::Ternary(TernaryFn::Clamp),
            "lerp" => Self::Ternary(TernaryFn::Lerp),
            "inverse_lerp" => Self::Ternary(TernaryFn::InverseLerp),
            "lerprotate" => Self::Ternary(TernaryFn::LerpRotate),
            "random" | "random_integer" => Self::Random(2),
            "die_roll" | "die_roll_integer" => Self::Random(3),
            _ => return Easing::parse(name).map(|easing| Self::Ternary(TernaryFn::Ease(easing))),
        };
        Some(found)
    }

    pub(crate) const fn arity(self) -> usize {
        match self {
            Self::Pi => 0,
            Self::Unary(_) => 1,
            Self::Binary(_) => 2,
            Self::Ternary(_) => 3,
            Self::Random(arity) => arity as usize,
        }
    }

    pub(crate) fn milestone(self) -> Milestone {
        match self {
            Self::Pi
            | Self::Unary(UnaryFn::Sin | UnaryFn::Cos)
            | Self::Ternary(TernaryFn::Clamp | TernaryFn::Lerp) => Milestone::One,
            Self::Random(_) => Milestone::Later,
            _ => Milestone::Two,
        }
    }
}

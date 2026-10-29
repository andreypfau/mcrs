use alloc::vec::Vec;

use mcrs_text_span::Span;

use crate::build::{ArrayKind, Build, Scalar};
use crate::error::{IntegerType, SnbtErrorKind};
use crate::reader::{ReadResult, Reader, Shape};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Signedness {
    Signed,
    Unsigned,
}

#[derive(Clone, Copy)]
struct Suffix {
    /// `None` when the suffix has no `s` or `u`, so the radix decides.
    signedness: Option<Signedness>,
    ty: IntegerType,
}

#[derive(Clone, Copy)]
pub(crate) struct IntLiteral<'a> {
    negative: bool,
    radix: u32,
    digits: &'a str,
    suffix: Option<Suffix>,
    span: Span,
}

struct FloatParts<'a> {
    whole: Option<&'a str>,
    fraction: Option<&'a str>,
    exponent: Option<Exponent<'a>>,
    float: bool,
}

#[derive(Clone, Copy)]
struct Exponent<'a> {
    negative: bool,
    digits: &'a str,
}

impl<'a, B: Build> Reader<'a, B> {
    fn run(&mut self, accept: fn(char) -> bool, empty: SnbtErrorKind) -> ReadResult<&'a str> {
        let start = self.pos;
        self.skip_whitespace();
        let at = self.pos;
        let rest = self.rest();
        let length = rest
            .find(|c: char| !(c == '_' || accept(c)))
            .unwrap_or(rest.len());
        let run = rest.get(..length).unwrap_or_default();
        let kind = if run.is_empty() {
            empty
        } else if run.starts_with('_') || run.ends_with('_') {
            SnbtErrorKind::UnderscoreNotAllowed
        } else {
            self.pos = at + length;
            return Ok(run);
        };
        self.pos = start;
        self.fail(at, kind)
    }

    fn decimal(&mut self) -> ReadResult<&'a str> {
        self.run(
            |c| c.is_ascii_digit(),
            SnbtErrorKind::ExpectedDecimalNumeral,
        )
    }

    fn sign(&mut self) -> ReadResult<bool> {
        Ok(self.opt(|r| r.first_token(&["+", "-"]))? == Some('-'))
    }

    fn exponent(&mut self) -> ReadResult<Exponent<'a>> {
        self.token("e|E")?;
        let negative = self.sign()?;
        Ok(Exponent {
            negative,
            digits: self.decimal()?,
        })
    }

    fn float_suffix(&mut self) -> ReadResult<bool> {
        Ok(self
            .first_token(&["f|F", "d|D"])?
            .eq_ignore_ascii_case(&'f'))
    }

    fn float_tail(
        &mut self,
        whole: Option<&'a str>,
        fraction: Option<&'a str>,
    ) -> ReadResult<FloatParts<'a>> {
        let exponent = self.opt(Self::exponent)?;
        let float = self.opt(Self::float_suffix)? == Some(true);
        Ok(FloatParts {
            whole,
            fraction,
            exponent,
            float,
        })
    }

    fn float_body(&mut self) -> ReadResult<FloatParts<'a>> {
        let start = self.pos;
        if let Some(whole) = self.opt(Self::decimal)?
            && self.opt(|r| r.token("."))?.is_some()
        {
            let fraction = self.opt(Self::decimal)?;
            return self.float_tail(Some(whole), fraction);
        }
        self.pos = start;
        if self.opt(|r| r.token("."))?.is_some() {
            let fraction = self.decimal()?;
            return self.float_tail(None, Some(fraction));
        }
        self.pos = start;
        let whole = self.decimal()?;
        let exponent = self.opt(Self::exponent)?;
        let float = if exponent.is_some() {
            self.opt(Self::float_suffix)? == Some(true)
        } else {
            self.float_suffix()?
        };
        Ok(FloatParts {
            whole: Some(whole),
            fraction: None,
            exponent,
            float,
        })
    }

    pub(crate) fn float(&mut self) -> ReadResult<(B::Value, Shape<'a>)> {
        let negative = self.sign()?;
        let parts = self.float_body()?;
        match float_value(negative, &parts) {
            Some((scalar, nonzero)) => Ok((self.build.scalar(scalar), Shape::Number { nonzero })),
            None => self.fail(self.pos, SnbtErrorKind::InfinityNotAllowed),
        }
    }

    fn type_letter(&mut self) -> ReadResult<IntegerType> {
        Ok(match self.first_token(&["b|B", "s|S", "i|I", "l|L"])? {
            'b' | 'B' => IntegerType::Byte,
            's' | 'S' => IntegerType::Short,
            'i' | 'I' => IntegerType::Int,
            _ => IntegerType::Long,
        })
    }

    fn integer_suffix(&mut self) -> ReadResult<Option<Suffix>> {
        for (mark, signedness) in [("u|U", Signedness::Unsigned), ("s|S", Signedness::Signed)] {
            let start = self.pos;
            if self.opt(|r| r.token(mark))?.is_some() {
                if let Some(ty) = self.opt(Self::type_letter)? {
                    return Ok(Some(Suffix {
                        signedness: Some(signedness),
                        ty,
                    }));
                }
                self.pos = start;
            }
        }
        Ok(self.opt(Self::type_letter)?.map(|ty| Suffix {
            signedness: None,
            ty,
        }))
    }

    fn after_zero(&mut self) -> ReadResult<(u32, &'a str)> {
        if self.opt(|r| r.token("x|X"))?.is_some() {
            return Ok((
                16,
                self.run(|c| c.is_ascii_hexdigit(), SnbtErrorKind::ExpectedHexNumeral)?,
            ));
        }
        let binary = self.opt(|r| {
            r.token("b|B")?;
            r.run(
                |c| matches!(c, '0' | '1'),
                SnbtErrorKind::ExpectedBinaryNumeral,
            )
        })?;
        if let Some(digits) = binary {
            return Ok((2, digits));
        }
        if self.opt(Self::decimal)?.is_some() {
            return self.fail(self.pos, SnbtErrorKind::LeadingZeroNotAllowed);
        }
        Ok((10, "0"))
    }

    pub(crate) fn integer(&mut self) -> ReadResult<IntLiteral<'a>> {
        self.skip_whitespace();
        let body = self.pos;
        let negative = self.sign()?;
        let (radix, digits) = if self.opt(|r| r.token("0"))?.is_some() {
            self.after_zero()?
        } else {
            (10, self.decimal()?)
        };
        let suffix = self.integer_suffix()?;
        Ok(IntLiteral {
            negative,
            radix,
            digits,
            suffix,
            span: Span {
                start: body,
                end: self.pos,
            },
        })
    }

    pub(crate) fn check_integer(&mut self, literal: IntLiteral<'a>) -> ReadResult<(Scalar, bool)> {
        let ty = literal.suffix.map_or(IntegerType::Int, |suffix| suffix.ty);
        match integer_value(literal, ty) {
            Ok(value) => Ok((Scalar::integer(ty, value), value != 0)),
            Err(kind) => self.fail(self.pos, kind),
        }
    }
}

/// SNBT-VAL-003: the value of an integer literal as `ty`, sign-extended to 64 bits.
fn integer_value(literal: IntLiteral<'_>, ty: IntegerType) -> Result<i64, SnbtErrorKind> {
    let signed = literal
        .suffix
        .and_then(|suffix| suffix.signedness)
        .map_or(literal.radix == 10, |signedness| {
            signedness == Signedness::Signed
        });
    if !signed && literal.negative {
        return Err(SnbtErrorKind::ExpectedNonNegativeNumber);
    }
    let failure = SnbtErrorKind::NumberParseFailure {
        number: literal.span,
        ty,
    };
    let mut magnitude: u128 = 0;
    for digit in literal
        .digits
        .chars()
        .filter_map(|c| c.to_digit(literal.radix))
    {
        magnitude = magnitude
            .checked_mul(u128::from(literal.radix))
            .and_then(|m| m.checked_add(u128::from(digit)))
            .filter(|&m| m <= u128::from(u64::MAX))
            .ok_or(failure)?;
    }
    let bits = ty.bits();
    let limit: u128 = match (signed, literal.negative) {
        (true, true) => 1 << (bits - 1),
        (true, false) => (1 << (bits - 1)) - 1,
        (false, _) => (1 << bits) - 1,
    };
    if magnitude > limit {
        return Err(failure);
    }
    let raw = if literal.negative {
        (magnitude as i128).wrapping_neg() as i64
    } else {
        magnitude as u64 as i64
    };
    let shift = 64 - bits;
    Ok((raw << shift) >> shift)
}

/// SNBT-VAL-005: the type of an element is its own, or the type of the array.
pub(crate) fn array_element(
    literal: IntLiteral<'_>,
    kind: ArrayKind,
) -> Result<i64, SnbtErrorKind> {
    let ty = literal.suffix.map_or(kind.element(), |suffix| suffix.ty);
    let allowed = match kind {
        ArrayKind::Byte => matches!(ty, IntegerType::Byte),
        ArrayKind::Int => !matches!(ty, IntegerType::Long),
        ArrayKind::Long => true,
    };
    if !allowed {
        return Err(SnbtErrorKind::InvalidArrayElementType);
    }
    integer_value(literal, ty)
}

const FLOAT_TEXT: usize = 128;

/// SNBT-VAL-004: the value rounded once from the decimal text, or `None` when it is infinite.
/// The value and whether it is not zero.
fn float_value(negative: bool, parts: &FloatParts<'_>) -> Option<(Scalar, bool)> {
    let mut stack = [0u8; FLOAT_TEXT];
    let mut heap = Vec::new();
    let pieces = [
        if negative { "-" } else { "" },
        parts.whole.unwrap_or("0"),
        ".",
        parts.fraction.unwrap_or("0"),
        if parts.exponent.is_some() { "e" } else { "" },
        match parts.exponent {
            Some(Exponent { negative: true, .. }) => "-",
            _ => "",
        },
        parts.exponent.map_or("", |exponent| exponent.digits),
    ];
    let length: usize = pieces.iter().map(|piece| piece.len()).sum();
    let bytes = pieces
        .iter()
        .flat_map(|piece| piece.bytes())
        .filter(|&b| b != b'_');
    // chisle: a number text longer than FLOAT_TEXT bytes allocates once; a decimal reader over
    // the pieces lifts it.
    let text: &[u8] = if length <= FLOAT_TEXT {
        let mut used = 0;
        for (slot, byte) in stack.iter_mut().zip(bytes) {
            *slot = byte;
            used += 1;
        }
        stack.get(..used)?
    } else {
        heap.extend(bytes);
        &heap
    };
    let text = core::str::from_utf8(text).ok()?;
    Some(if parts.float {
        let value = text.parse::<f32>().ok().filter(|v| v.is_finite())? + 0.0;
        (Scalar::Float(value), value != 0.0)
    } else {
        let value = text.parse::<f64>().ok().filter(|v| v.is_finite())? + 0.0;
        (Scalar::Double(value), value != 0.0)
    })
}

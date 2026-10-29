use alloc::vec::Vec;

use mcrs_text_span::Span;

use crate::build::{ArrayKind, Build, Scalar};
use crate::error::{IntegerType, SnbtErrorKind as K};
use crate::reader::{R, Reader, Shape};

#[derive(Clone, Copy)]
struct Suffix {
    signed: Option<bool>,
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
    exponent: Option<(bool, &'a str)>,
    float: bool,
}

impl<'a, B: Build> Reader<'a, B> {
    fn run(&mut self, accept: fn(char) -> bool, empty: K) -> R<&'a str> {
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
            K::UnderscoreNotAllowed
        } else {
            self.pos = at + length;
            return Ok(run);
        };
        self.pos = start;
        self.fail(at, kind)
    }

    fn decimal(&mut self) -> R<&'a str> {
        self.run(|c| c.is_ascii_digit(), K::ExpectedDecimalNumeral)
    }

    fn sign(&mut self) -> R<bool> {
        Ok(self.opt(|r| r.first_token(&["+", "-"]))? == Some('-'))
    }

    fn exponent(&mut self) -> R<(bool, &'a str)> {
        self.token("e|E")?;
        let negative = self.sign()?;
        Ok((negative, self.decimal()?))
    }

    fn float_suffix(&mut self) -> R<bool> {
        Ok(self
            .first_token(&["f|F", "d|D"])?
            .eq_ignore_ascii_case(&'f'))
    }

    fn float_tail(
        &mut self,
        whole: Option<&'a str>,
        fraction: Option<&'a str>,
    ) -> R<FloatParts<'a>> {
        let exponent = self.opt(Self::exponent)?;
        let float = self.opt(Self::float_suffix)? == Some(true);
        Ok(FloatParts {
            whole,
            fraction,
            exponent,
            float,
        })
    }

    fn float_body(&mut self) -> R<FloatParts<'a>> {
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

    pub(crate) fn float(&mut self) -> R<(B::Value, Shape<'a>)> {
        let negative = self.sign()?;
        let parts = self.float_body()?;
        match float_value(negative, &parts) {
            Some((scalar, nonzero)) => Ok((self.build.scalar(scalar), Shape::Number { nonzero })),
            None => self.fail(self.pos, K::InfinityNotAllowed),
        }
    }

    fn type_letter(&mut self) -> R<IntegerType> {
        Ok(match self.first_token(&["b|B", "s|S", "i|I", "l|L"])? {
            'b' | 'B' => IntegerType::Byte,
            's' | 'S' => IntegerType::Short,
            'i' | 'I' => IntegerType::Int,
            _ => IntegerType::Long,
        })
    }

    fn integer_suffix(&mut self) -> R<Option<Suffix>> {
        for (mark, signed) in [("u|U", false), ("s|S", true)] {
            let start = self.pos;
            if self.opt(|r| r.token(mark))?.is_some() {
                if let Some(ty) = self.opt(Self::type_letter)? {
                    return Ok(Some(Suffix {
                        signed: Some(signed),
                        ty,
                    }));
                }
                self.pos = start;
            }
        }
        Ok(self
            .opt(Self::type_letter)?
            .map(|ty| Suffix { signed: None, ty }))
    }

    fn after_zero(&mut self) -> R<(u32, &'a str)> {
        if self.opt(|r| r.token("x|X"))?.is_some() {
            return Ok((
                16,
                self.run(|c| c.is_ascii_hexdigit(), K::ExpectedHexNumeral)?,
            ));
        }
        let binary = self.opt(|r| {
            r.token("b|B")?;
            r.run(|c| matches!(c, '0' | '1'), K::ExpectedBinaryNumeral)
        })?;
        if let Some(digits) = binary {
            return Ok((2, digits));
        }
        if self.opt(Self::decimal)?.is_some() {
            return self.fail(self.pos, K::LeadingZeroNotAllowed);
        }
        Ok((10, "0"))
    }

    pub(crate) fn integer(&mut self) -> R<IntLiteral<'a>> {
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

    pub(crate) fn check_integer(&mut self, literal: IntLiteral<'a>) -> R<(Scalar, bool)> {
        let ty = literal.suffix.map_or(IntegerType::Int, |suffix| suffix.ty);
        match integer_value(literal, ty) {
            Ok(value) => Ok((Scalar::integer(ty, value), value != 0)),
            Err(kind) => self.fail(self.pos, kind),
        }
    }
}

/// SNBT-VAL-003: the value of an integer literal as `ty`, sign-extended to 64 bits.
fn integer_value(literal: IntLiteral<'_>, ty: IntegerType) -> Result<i64, K> {
    let signed = literal
        .suffix
        .and_then(|suffix| suffix.signed)
        .unwrap_or(literal.radix == 10);
    if !signed && literal.negative {
        return Err(K::ExpectedNonNegativeNumber);
    }
    let failure = K::NumberParseFailure {
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
pub(crate) fn array_element(literal: IntLiteral<'_>, kind: ArrayKind) -> Result<i64, K> {
    let ty = literal.suffix.map_or(kind.element(), |suffix| suffix.ty);
    let allowed = match kind {
        ArrayKind::Byte => matches!(ty, IntegerType::Byte),
        ArrayKind::Int => !matches!(ty, IntegerType::Long),
        ArrayKind::Long => true,
    };
    if !allowed {
        return Err(K::InvalidArrayElementType);
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
            Some((true, _)) => "-",
            _ => "",
        },
        parts.exponent.map_or("", |(_, digits)| digits),
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

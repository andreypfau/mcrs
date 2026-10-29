use crate::error::{LibraryError, SyntaxError};
use crate::parse::ContextView;
use crate::reader::{Reader, StringSpan, StringValue};
use crate::suggest::{SuggestionBuilder, starts_with_ignore_case};
use core::convert::Infallible;

/// An argument type: how it reads a value from the command line.
pub trait Argument<S> {
    /// The value read, which may borrow the command line.
    type Value<'a>;
    /// The errors of this type beside those of the library.
    type Error;

    /// Reads a value at the reader's cursor and leaves the cursor after it.
    fn parse<'a>(
        &self,
        reader: &mut Reader<'a>,
        source: &S,
    ) -> Result<Self::Value<'a>, SyntaxError<Self::Error>>;
}

/// The argument types of one tree, composed by the application into one enum.
pub trait ArgumentSet<S> {
    /// The errors of every argument type of the set.
    type Error: Copy;
    /// The tooltip a suggestion of the set carries.
    type Tooltip: PartialEq;

    /// Reads a value and keeps only whether it was valid.
    fn check(&self, reader: &mut Reader<'_>, source: &S) -> Result<(), SyntaxError<Self::Error>>;

    /// Suggests at the builder's start; `context` holds the nodes parsed before it, in the
    /// context the suggesting node belongs to.
    fn suggest<'t>(
        &'t self,
        source: &'t S,
        context: &ContextView<'_>,
        builder: &mut SuggestionBuilder<'t, '_, Self::Tooltip>,
    );
}

/// `true` or `false`, quoted or not.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BoolArgument;

impl BoolArgument {
    /// Suggests `true` and `false` where they match the remaining input.
    pub fn suggest<'t, T>(builder: &mut SuggestionBuilder<'t, '_, T>) {
        builder.suggest_matching(["true", "false"], starts_with_ignore_case);
    }
}

impl<S> Argument<S> for BoolArgument {
    type Value<'a> = bool;
    type Error = Infallible;

    fn parse<'a>(&self, reader: &mut Reader<'a>, _: &S) -> Result<bool, SyntaxError<Infallible>> {
        reader.read_bool()
    }
}

/// A number type an argument reads, with the errors of its bounds.
pub trait Number: Copy + PartialOrd {
    const MIN: Self;
    const MAX: Self;

    fn read<E>(reader: &mut Reader<'_>) -> Result<Self, SyntaxError<E>>;

    fn too_low(value: Self, min: Self) -> LibraryError;

    fn too_high(value: Self, max: Self) -> LibraryError;
}

impl Number for i32 {
    const MIN: Self = i32::MIN;
    const MAX: Self = i32::MAX;

    fn read<E>(reader: &mut Reader<'_>) -> Result<Self, SyntaxError<E>> {
        reader.read_int()
    }

    fn too_low(value: Self, min: Self) -> LibraryError {
        LibraryError::IntegerTooLow { value, min }
    }

    fn too_high(value: Self, max: Self) -> LibraryError {
        LibraryError::IntegerTooHigh { value, max }
    }
}

impl Number for i64 {
    const MIN: Self = i64::MIN;
    const MAX: Self = i64::MAX;

    fn read<E>(reader: &mut Reader<'_>) -> Result<Self, SyntaxError<E>> {
        reader.read_long()
    }

    fn too_low(value: Self, min: Self) -> LibraryError {
        LibraryError::LongTooLow { value, min }
    }

    fn too_high(value: Self, max: Self) -> LibraryError {
        LibraryError::LongTooHigh { value, max }
    }
}

impl Number for f32 {
    const MIN: Self = f32::MIN;
    const MAX: Self = f32::MAX;

    fn read<E>(reader: &mut Reader<'_>) -> Result<Self, SyntaxError<E>> {
        reader.read_float()
    }

    fn too_low(value: Self, min: Self) -> LibraryError {
        LibraryError::FloatTooLow { value, min }
    }

    fn too_high(value: Self, max: Self) -> LibraryError {
        LibraryError::FloatTooHigh { value, max }
    }
}

impl Number for f64 {
    const MIN: Self = f64::MIN;
    const MAX: Self = f64::MAX;

    fn read<E>(reader: &mut Reader<'_>) -> Result<Self, SyntaxError<E>> {
        reader.read_double()
    }

    fn too_low(value: Self, min: Self) -> LibraryError {
        LibraryError::DoubleTooLow { value, min }
    }

    fn too_high(value: Self, max: Self) -> LibraryError {
        LibraryError::DoubleTooHigh { value, max }
    }
}

/// A number between `min` and `max`, both inclusive.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NumberArgument<T> {
    pub min: T,
    pub max: T,
}

pub type IntegerArgument = NumberArgument<i32>;
pub type LongArgument = NumberArgument<i64>;
pub type FloatArgument = NumberArgument<f32>;
pub type DoubleArgument = NumberArgument<f64>;

impl<T: Number> NumberArgument<T> {
    pub const ANY: Self = Self {
        min: T::MIN,
        max: T::MAX,
    };

    /// The minimum and the maximum, each where it differs from [`Self::ANY`].
    pub fn limits(&self) -> (Option<T>, Option<T>) {
        (
            (self.min != T::MIN).then_some(self.min),
            (self.max != T::MAX).then_some(self.max),
        )
    }
}

impl<S, T: Number> Argument<S> for NumberArgument<T> {
    type Value<'a> = T;
    type Error = Infallible;

    fn parse<'a>(&self, reader: &mut Reader<'a>, _: &S) -> Result<T, SyntaxError<Infallible>> {
        let start = reader.cursor();
        let value = T::read(reader)?;
        let error = if value < self.min {
            T::too_low(value, self.min)
        } else if value > self.max {
            T::too_high(value, self.max)
        } else {
            return Ok(value);
        };
        reader.set_cursor(start);
        Err(SyntaxError::library(error, start))
    }
}

/// A string argument, by how much of the command line it takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StringArgument {
    /// One unquoted word.
    Word,
    /// One unquoted word or one quoted string.
    Phrase,
    /// The rest of the command line.
    Greedy,
}

impl<S> Argument<S> for StringArgument {
    type Value<'a> = StringValue<'a>;
    type Error = Infallible;

    fn parse<'a>(
        &self,
        reader: &mut Reader<'a>,
        _: &S,
    ) -> Result<StringValue<'a>, SyntaxError<Infallible>> {
        let string = match self {
            StringArgument::Word => StringSpan {
                span: reader.read_unquoted(),
                quoted: false,
            },
            StringArgument::Phrase => reader.read_string()?,
            StringArgument::Greedy => {
                let start = reader.cursor();
                reader.set_cursor(reader.input().len());
                StringSpan {
                    span: reader.span_from(start),
                    quoted: false,
                }
            }
        };
        Ok(StringValue::new(reader.input(), string))
    }
}

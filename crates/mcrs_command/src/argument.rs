use crate::error::{LibraryError, StringSpan, SyntaxError};
use crate::parse::ContextView;
use crate::reader::{Reader, StringValue};
use crate::suggest::{SuggestionBuilder, starts_with_ignore_case};
use core::convert::Infallible;

/// An argument type: how it reads a value from the command line.
pub trait Argument<S> {
    type Value<'a>;
    type Error;

    fn parse<'a>(
        &self,
        reader: &mut Reader<'a>,
        source: &S,
    ) -> Result<Self::Value<'a>, SyntaxError<Self::Error>>;
}

/// The argument types of one tree, composed by the application into one enum.
pub trait ArgumentSet<S> {
    type Error: Copy;
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BoolArgument;

impl BoolArgument {
    pub fn suggest<'t, T>(builder: &mut SuggestionBuilder<'t, '_, T>) {
        builder.suggest_matching(["true", "false"], |remaining, word| {
            starts_with_ignore_case(word, remaining)
        });
    }
}

impl<S> Argument<S> for BoolArgument {
    type Value<'a> = bool;
    type Error = Infallible;

    fn parse<'a>(&self, reader: &mut Reader<'a>, _: &S) -> Result<bool, SyntaxError<Infallible>> {
        reader.read_bool()
    }
}

macro_rules! number_argument {
    ($name:ident, $ty:ty, $read:ident, $low:ident, $high:ident) => {
        #[derive(Clone, Copy, Debug, PartialEq)]
        pub struct $name {
            pub min: $ty,
            pub max: $ty,
        }

        impl $name {
            pub const ANY: Self = Self {
                min: <$ty>::MIN,
                max: <$ty>::MAX,
            };

            /// The minimum and the maximum, each where it differs from [`Self::ANY`].
            pub fn limits(&self) -> (Option<$ty>, Option<$ty>) {
                (
                    (self.min != Self::ANY.min).then_some(self.min),
                    (self.max != Self::ANY.max).then_some(self.max),
                )
            }
        }

        impl<S> Argument<S> for $name {
            type Value<'a> = $ty;
            type Error = Infallible;

            fn parse<'a>(
                &self,
                reader: &mut Reader<'a>,
                _: &S,
            ) -> Result<$ty, SyntaxError<Infallible>> {
                let start = reader.cursor();
                let value = reader.$read()?;
                let error = if value < self.min {
                    LibraryError::$low {
                        value,
                        min: self.min,
                    }
                } else if value > self.max {
                    LibraryError::$high {
                        value,
                        max: self.max,
                    }
                } else {
                    return Ok(value);
                };
                reader.set_cursor(start);
                Err(SyntaxError::library(error, start))
            }
        }
    };
}

number_argument!(
    IntegerArgument,
    i32,
    read_int,
    IntegerTooLow,
    IntegerTooHigh
);
number_argument!(LongArgument, i64, read_long, LongTooLow, LongTooHigh);
number_argument!(FloatArgument, f32, read_float, FloatTooLow, FloatTooHigh);
number_argument!(
    DoubleArgument,
    f64,
    read_double,
    DoubleTooLow,
    DoubleTooHigh
);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StringArgument {
    Word,
    Phrase,
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

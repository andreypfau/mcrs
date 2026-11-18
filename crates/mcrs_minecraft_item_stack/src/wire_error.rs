use core::error::Error;
use core::fmt;

use mcrs_minecraft_wire::ErrorKind;

use crate::BuiltinKey;
use crate::ComponentType;
use crate::NameTable;
use crate::SavedError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum WireError {
    UnexpectedEnd,
    Malformed,
    CountTooLarge,
    CountOutOfRange,
    EmptyStack,
    UnknownItem,
    UnknownComponentType(i32),
    UnsupportedComponentType(ComponentType),
    UnregisteredType,
    RepeatedComponentType(ComponentType),
    BadLength(ComponentType),
    TooLong,
    TooManyEntries,
    BadValue(BuiltinKey),
    OutOfRange(BuiltinKey),
    UnknownEntry {
        component: BuiltinKey,
        table: NameTable,
    },
    UnnamedId(NameTable),
    SavedValueOnly,
    TooDeep,
    QuotaExceeded,
}

impl WireError {
    pub(crate) fn of_frame(error: &mcrs_minecraft_wire::Error) -> Self {
        match error.kind() {
            ErrorKind::UnexpectedEnd => Self::UnexpectedEnd,
            _ => Self::Malformed,
        }
    }

    pub(crate) fn of_value(error: &mcrs_minecraft_wire::Error, component: BuiltinKey) -> Self {
        match error.kind() {
            ErrorKind::UnexpectedEnd => Self::UnexpectedEnd,
            _ => Self::BadValue(component),
        }
    }

    pub(crate) fn of_saved(error: SavedError, component: BuiltinKey) -> Self {
        match error {
            SavedError::OutOfRange(component) => Self::OutOfRange(component),
            SavedError::TooDeep => Self::TooDeep,
            SavedError::UnknownEntry { component, table } => {
                Self::UnknownEntry { component, table }
            }
            _ => Self::BadValue(component),
        }
    }
}

impl fmt::Display for WireError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEnd => formatter.write_str("the input ends inside the stack"),
            Self::Malformed => formatter.write_str("the frame of the stack is malformed"),
            Self::CountTooLarge => formatter.write_str("the count is larger than 255"),
            Self::CountOutOfRange => formatter.write_str("the count is larger than 99"),
            Self::EmptyStack => formatter.write_str("the format has no form for the empty stack"),
            Self::UnknownItem => formatter.write_str("the item id names no item"),
            Self::UnknownComponentType(id) => {
                write!(formatter, "the type id {id} names no component type")
            }
            Self::UnsupportedComponentType(component_type) => write!(
                formatter,
                "the component type {} has no wire form in this crate",
                component_type.0
            ),
            Self::UnregisteredType => {
                formatter.write_str("the type of a patch entry has no type id")
            }
            Self::RepeatedComponentType(component_type) => write!(
                formatter,
                "the patch holds the component type {} two times",
                component_type.0
            ),
            Self::BadLength(component_type) => write!(
                formatter,
                "a component of type {} does not match its length prefix",
                component_type.0
            ),
            Self::TooLong => formatter.write_str("a component or a list is too long for a VarInt"),
            Self::TooManyEntries => {
                formatter.write_str("a list of the hashed stack has more than 256 entries")
            }
            Self::BadValue(component) => write!(
                formatter,
                "the value of {} is not a valid wire form",
                component.path()
            ),
            Self::OutOfRange(component) => write!(
                formatter,
                "a number of {} is out of range",
                component.path()
            ),
            Self::UnknownEntry { component, table } => write!(
                formatter,
                "the value of {} names an unknown entry of {table:?}",
                component.path()
            ),
            Self::UnnamedId(table) => write!(formatter, "a numeric id has no name in {table:?}"),
            Self::SavedValueOnly => formatter
                .write_str("the patch holds an undecoded saved component, which has no wire form"),
            Self::TooDeep => formatter.write_str("the saved form of the stack is nested too deep"),
            Self::QuotaExceeded => formatter.write_str("the network NBT exceeds its budget"),
        }
    }
}

impl Error for WireError {}

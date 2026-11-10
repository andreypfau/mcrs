use core::error::Error;
use core::fmt;

use mcrs_minecraft_wire::ErrorKind;

use crate::BuiltinKey;
use crate::ComponentType;
use crate::NameTable;

/// Why the wire form of a stack was not read or not written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum WireError {
    /// The input ends inside the stack.
    UnexpectedEnd,
    /// A VarInt of the frame is longer than five bytes, or a number of patch entries is negative.
    Malformed,
    /// The count is larger than 255, the largest count of a stack.
    CountTooLarge,
    /// The count is larger than 99. The stack has no saved form, so the format refuses it.
    CountOutOfRange,
    /// The stack is empty, and the format has no form for the empty stack.
    EmptyStack,
    /// The item id names no item.
    UnknownItem,
    /// The type id names no component type.
    UnknownComponentType(i32),
    /// A component of this type has no length prefix, and this crate cannot read its wire form.
    UnsupportedComponentType(ComponentType),
    /// The table of component types has no type id for the type of a patch entry.
    UnregisteredType,
    /// The patch holds two entries for this component type.
    RepeatedComponentType(ComponentType),
    /// A component does not use the number of bytes that its length prefix states.
    BadLength(ComponentType),
    /// A component or a list is longer than a VarInt can state.
    TooLong,
    /// A list of entries of a hashed stack has more than 256 entries.
    TooManyEntries,
    /// The value of this component type holds an id out of its list, a negative number of
    /// elements, a boolean byte that is not `00` or `01`, a string that is not an identifier, or
    /// one enchantment two times.
    BadValue(BuiltinKey),
    /// A number of this component type is out of the range of its saved form.
    OutOfRange(BuiltinKey),
    /// The value of this component type names an entry that the table does not have.
    UnknownEntry {
        component: BuiltinKey,
        table: NameTable,
    },
    /// The stack holds a numeric id that has no name in the table.
    UnnamedId(NameTable),
    /// This component holds a text, and this crate has no wire form of a text yet.
    TextNotSupported(BuiltinKey),
    /// The patch holds the saved form of a component that this crate does not decode. That form
    /// has no wire form here.
    SavedValueOnly,
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
            Self::TextNotSupported(component) => write!(
                formatter,
                "the value of {} holds a text, which has no wire form yet",
                component.path()
            ),
            Self::SavedValueOnly => formatter
                .write_str("the patch holds an undecoded saved component, which has no wire form"),
        }
    }
}

impl Error for WireError {}

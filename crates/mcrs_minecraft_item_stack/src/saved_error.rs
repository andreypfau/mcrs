use core::error::Error;
use core::fmt;

use crate::BuiltinKey;
use crate::ComponentType;
use crate::NameTable;

/// Why the saved form of a stack was not read or not written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum SavedError {
    /// `id` is absent, or is not a string that holds an identifier.
    BadItemId,
    /// `id` names no item.
    UnknownItem,
    /// `id` names air. The saved stack has no form for the empty stack.
    AirItem,
    /// `count` is not an int, or is out of `1..=99`.
    BadCount,
    /// `Slot` is not a byte.
    BadSlot,
    /// `components` is not a map.
    BadPatch,
    /// A key of the patch is not an identifier, with or without the removal marker `!`.
    BadKey,
    /// A key of the patch names no component type.
    UnknownComponentType,
    /// The value of this component type has a tag of the wrong kind, lacks a necessary field,
    /// holds a string that is not a name of its list, or holds one entry two times.
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
    /// This component holds a text, and this crate has no saved form of a text yet.
    TextNotSupported(BuiltinKey),
    /// The bytes of an undecoded patch entry are not one NBT value, or an NBT value of the input
    /// has no bytes.
    BadUndecodedBytes,
    /// The patch holds only the wire form of a component of this type, which has no saved form
    /// here.
    WireValueOnly(ComponentType),
}

impl fmt::Display for SavedError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadItemId => formatter.write_str("the item id is absent or is not an identifier"),
            Self::UnknownItem => formatter.write_str("the item id names no item"),
            Self::AirItem => formatter.write_str("the item id names air"),
            Self::BadCount => formatter.write_str("the count is not an int from 1 to 99"),
            Self::BadSlot => formatter.write_str("the slot is not a byte"),
            Self::BadPatch => formatter.write_str("the component patch is not a map"),
            Self::BadKey => {
                formatter.write_str("a key of the component patch is not an identifier")
            }
            Self::UnknownComponentType => {
                formatter.write_str("a key of the component patch names no component type")
            }
            Self::BadValue(component) => write!(
                formatter,
                "the value of {} is not a valid saved form",
                component.path()
            ),
            Self::OutOfRange(component) => {
                write!(
                    formatter,
                    "a number of {} is out of range",
                    component.path()
                )
            }
            Self::UnknownEntry { component, table } => write!(
                formatter,
                "the value of {} names an unknown entry of {table:?}",
                component.path()
            ),
            Self::UnnamedId(table) => write!(formatter, "a numeric id has no name in {table:?}"),
            Self::TextNotSupported(component) => write!(
                formatter,
                "the value of {} holds a text, which has no saved form yet",
                component.path()
            ),
            Self::BadUndecodedBytes => {
                formatter.write_str("the bytes of an undecoded patch entry are not one NBT value")
            }
            Self::WireValueOnly(component_type) => write!(
                formatter,
                "the component type {} has only an undecoded wire form",
                component_type.0
            ),
        }
    }
}

impl Error for SavedError {}

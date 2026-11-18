use core::error::Error;
use core::fmt;

use crate::BuiltinKey;
use crate::ComponentType;
use crate::NameTable;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum SavedError {
    BadItemId,
    UnknownItem,
    AirItem,
    BadCount,
    BadSlot,
    BadPatch,
    BadKey,
    UnknownComponentType,
    BadRemovalValue,
    RepeatedKey,
    BadValue(BuiltinKey),
    OutOfRange(BuiltinKey),
    UnknownEntry {
        component: BuiltinKey,
        table: NameTable,
    },
    UnnamedId(NameTable),
    BadUndecodedBytes,
    WireValueOnly(ComponentType),
    TooDeep,
}

impl fmt::Display for SavedError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadItemId => formatter.write_str("the item id is absent or is not an identifier"),
            Self::UnknownItem => formatter.write_str("the item id names no item"),
            Self::AirItem => formatter.write_str("the item id names air"),
            Self::BadCount => formatter.write_str("the count is not a number from 1 to 99"),
            Self::BadSlot => formatter.write_str("the slot is not a byte"),
            Self::BadPatch => formatter.write_str("the component patch is not a map"),
            Self::BadKey => {
                formatter.write_str("a key of the component patch is not an identifier")
            }
            Self::UnknownComponentType => {
                formatter.write_str("a key of the component patch names no component type")
            }
            Self::BadRemovalValue => formatter
                .write_str("the value of a removal key of the component patch is not a map"),
            Self::RepeatedKey => formatter
                .write_str("two keys of the component patch of one kind name one component type"),
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
            Self::BadUndecodedBytes => {
                formatter.write_str("the bytes of an undecoded patch entry are not one NBT value")
            }
            Self::WireValueOnly(component_type) => write!(
                formatter,
                "the component type {} has only an undecoded wire form",
                component_type.0
            ),
            Self::TooDeep => formatter.write_str("the saved form of the stack is nested too deep"),
        }
    }
}

impl Error for SavedError {}

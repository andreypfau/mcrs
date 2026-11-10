use core::error::Error;
use core::fmt;

use crate::BuiltinKey;
use crate::ComponentType;
use crate::NameTable;

/// Why the hash of a stack was not computed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum HashError {
    /// The item id of the stack names no item.
    UnknownItem,
    /// The table of component types has no type id for the type of a patch entry.
    UnregisteredType,
    /// This crate holds the component of this type only as bytes, so it has no tree to hash.
    UndecodedComponent(ComponentType),
    /// The value of this component type has no saved form to hash.
    BadValue(BuiltinKey),
    /// A number of this component type is out of the range of its saved form.
    OutOfRange(BuiltinKey),
    /// The value of this component type holds a numeric id that has no name in the table.
    UnknownEntry {
        component: BuiltinKey,
        table: NameTable,
    },
}

impl fmt::Display for HashError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownItem => formatter.write_str("the item id names no item"),
            Self::UnregisteredType => {
                formatter.write_str("the type of a patch entry has no type id")
            }
            Self::UndecodedComponent(component_type) => write!(
                formatter,
                "the component of type {} is not decoded and has no hash here",
                component_type.0
            ),
            Self::BadValue(component) => write!(
                formatter,
                "the value of {} has no saved form",
                component.path()
            ),
            Self::OutOfRange(component) => write!(
                formatter,
                "a number of {} is out of range",
                component.path()
            ),
            Self::UnknownEntry { component, table } => write!(
                formatter,
                "the value of {} holds an id that has no name in {table:?}",
                component.path()
            ),
        }
    }
}

impl Error for HashError {}

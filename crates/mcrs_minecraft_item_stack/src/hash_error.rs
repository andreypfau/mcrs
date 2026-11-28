use core::error::Error;
use core::fmt;

use crate::BuiltinKey;
use crate::ComponentType;
use crate::NameTable;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum HashError {
    UnknownItem,
    UnregisteredType,
    UndecodedComponent(ComponentType),
    BadValue(BuiltinKey),
    OutOfRange(BuiltinKey),
    TooDeep,
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
            Self::TooDeep => formatter.write_str("the saved form of the stack is nested too deep"),
            Self::UnknownEntry { component, table } => write!(
                formatter,
                "the value of {} holds an id that has no name in {table:?}",
                component.path()
            ),
        }
    }
}

impl Error for HashError {}

use alloc::string::String;
use core::error::Error;
use core::fmt;

use mcrs_minecraft_item_stack::SavedError;
use mcrs_minecraft_resource::ResourceLocationBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum BakeError {
    Layout {
        file: String,
        line: usize,
        column: usize,
    },
    RepeatedId {
        item: ResourceLocationBuf,
        id: u16,
    },
    RepeatedItem(ResourceLocationBuf),
    MissingId(u16),
    Component {
        item: ResourceLocationBuf,
        component: ResourceLocationBuf,
        cause: SavedError,
    },
}

impl fmt::Display for BakeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Layout { file, line, column } => write!(
                formatter,
                "{file} is not an item definition (line {line}, column {column})"
            ),
            Self::RepeatedId { item, id } => {
                write!(formatter, "{item} has the item id {id} of an earlier file")
            }
            Self::RepeatedItem(item) => write!(formatter, "two files define {item}"),
            Self::MissingId(id) => write!(formatter, "no file has the item id {id}"),
            Self::Component {
                item,
                component,
                cause,
            } => write!(formatter, "the component {component} of {item}: {cause}"),
        }
    }
}

impl Error for BakeError {}

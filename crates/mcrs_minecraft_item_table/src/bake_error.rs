use alloc::string::String;
use core::error::Error;
use core::fmt;

use mcrs_minecraft_item_stack::SavedError;
use mcrs_minecraft_resource::ResourceLocationBuf;

/// Why the item definitions did not bake into a table.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum BakeError {
    /// The file is not JSON, or does not have the layout of an item definition.
    Layout {
        file: String,
        line: usize,
        column: usize,
    },
    /// Two files have the same item id.
    RepeatedId { item: ResourceLocationBuf, id: u16 },
    /// Two files have the same item identifier.
    RepeatedItem(ResourceLocationBuf),
    /// No file has this item id, but a file has a larger one.
    MissingId(u16),
    /// A component of the item is not a valid saved form, or names no component type.
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

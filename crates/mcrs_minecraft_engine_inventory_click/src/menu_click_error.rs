use core::error::Error;
use core::fmt;

use mcrs_minecraft_inventory_click::ClickError;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MenuClickError {
    NoOpenMenu,
    NoMenuDrag,
    Click(ClickError),
}

impl From<ClickError> for MenuClickError {
    fn from(error: ClickError) -> Self {
        Self::Click(error)
    }
}

impl fmt::Display for MenuClickError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoOpenMenu => formatter.write_str("the viewer has no open menu"),
            Self::NoMenuDrag => {
                formatter.write_str("the open menu has no drag; add MenuClickPlugin")
            }
            Self::Click(error) => error.fmt(formatter),
        }
    }
}

impl Error for MenuClickError {}

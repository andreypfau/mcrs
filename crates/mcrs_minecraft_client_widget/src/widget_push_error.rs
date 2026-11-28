use core::error::Error;
use core::fmt;

use crate::widget_id::WidgetId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum WidgetPushError {
    IdTaken(WidgetId),
    ContainerInArea(WidgetId),
}

impl fmt::Display for WidgetPushError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IdTaken(id) => write!(
                formatter,
                "the screen already has a widget with the id {}",
                id.0
            ),
            Self::ContainerInArea(id) => write!(
                formatter,
                "the widget {} is a scroll container and cannot be a member of a scroll area",
                id.0
            ),
        }
    }
}

impl Error for WidgetPushError {}

use core::error::Error;
use core::fmt;

use crate::widget_id::WidgetId;

/// The screen already has a widget with this id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WidgetIdTaken(pub WidgetId);

impl fmt::Display for WidgetIdTaken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "the screen already has a widget with the id {}",
            self.0.0
        )
    }
}

impl Error for WidgetIdTaken {}

use crate::widget_id::WidgetId;

/// The scroll area that holds a widget, and the row of a widget list that holds it; the bounds of
/// the widget are then at the scroll offset 0 (SCRL-AREA-003).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AreaSlot {
    /// The selection list, widget list or scroll panel.
    pub area: WidgetId,
    /// The row in a widget list; a scroll panel ignores it.
    pub row: usize,
}

use crate::widget_id::WidgetId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AreaSlot {
    pub area: WidgetId,
    pub row: usize,
}

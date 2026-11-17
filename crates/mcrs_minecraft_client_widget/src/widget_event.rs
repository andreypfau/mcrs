use crate::key_modifiers::KeyModifiers;
use crate::widget_id::WidgetId;

#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WidgetEvent {
    Activated {
        widget: WidgetId,
        modifiers: KeyModifiers,
    },
    ValueChanged {
        widget: WidgetId,
    },
    ValuePending {
        widget: WidgetId,
    },
    FocusChanged {
        focused: Option<WidgetId>,
    },
    SelectionChanged {
        widget: WidgetId,
        row: Option<usize>,
    },
    RowActivated {
        widget: WidgetId,
        row: usize,
    },
    TabChanged {
        widget: WidgetId,
        tab: usize,
    },
}

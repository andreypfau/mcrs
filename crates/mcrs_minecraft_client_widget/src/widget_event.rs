use crate::key_modifiers::KeyModifiers;
use crate::widget_id::WidgetId;

/// What an input call did; the screen resolves the widget by its id.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WidgetEvent {
    /// A button, an icon button or an image button runs its action (WID-BTN-003).
    Activated {
        widget: WidgetId,
        modifiers: KeyModifiers,
    },
    /// A checkbox, a cycle button or a slider has a new value, which applies now, and the screen
    /// sets the new caption; or an edit box has a new value (EDIT-API-001).
    ValueChanged { widget: WidgetId },
    /// A deferred slider has a new value that applies later; the screen sets only the new
    /// caption (WID-SLD-007).
    ValuePending { widget: WidgetId },
    /// The focus moved to a widget, or cleared.
    FocusChanged { focused: Option<WidgetId> },
    /// A selection list has a new selected row, or none (LIST-SEL-002, LIST-SEL-005).
    SelectionChanged {
        widget: WidgetId,
        row: Option<usize>,
    },
    /// A double click or Enter on a row of a selection list; the screen decides what it does
    /// (LIST-SEL-002, LIST-SEL-004).
    RowActivated { widget: WidgetId, row: usize },
    /// A tab bar has a new current tab; the screen plays the click sound and shows the widgets
    /// of the tab (TAB-CLICK-001, TAB-AREA-001).
    TabChanged { widget: WidgetId, tab: usize },
}

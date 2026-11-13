use alloc::string::String;

use crate::widget::{INACTIVE_CAPTION, WHITE};

/// A tab of a tab bar (TAB-BAR-001).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tab {
    /// The label of the tab.
    pub label: String,
    /// Whether the tab can become the current tab by a press, a key or the focus.
    pub active: bool,
}

impl Tab {
    /// The ARGB colour of the label, and of the line under the label of the current tab
    /// (TAB-MENU-003, TAB-MENU-004).
    #[must_use]
    pub const fn label_color(&self) -> u32 {
        if self.active { WHITE } else { INACTIVE_CAPTION }
    }
}

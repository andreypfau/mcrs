use alloc::string::String;

use crate::widget::{INACTIVE_CAPTION, WHITE};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tab {
    pub label: String,
    pub active: bool,
}

impl Tab {
    #[must_use]
    pub const fn label_color(&self) -> u32 {
        if self.active { WHITE } else { INACTIVE_CAPTION }
    }
}

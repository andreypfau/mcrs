use crate::item_model_asset::DisplayContext;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct ItemModelContext {
    pub display: DisplayContext,
}

impl ItemModelContext {
    #[must_use]
    pub const fn new(display: DisplayContext) -> Self {
        Self { display }
    }
}

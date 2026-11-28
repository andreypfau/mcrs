#[derive(Clone, Copy)]
pub(crate) enum ClickPurpose {
    Click = 0,
    Hand = 1,
    Creative = 2,
}

impl ClickPurpose {
    pub(crate) const fn serial(self) -> u32 {
        self as u32
    }
}

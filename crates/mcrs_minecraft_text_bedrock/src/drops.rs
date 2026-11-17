#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct BedrockDrops {
    pub click_events: usize,
    pub hover_events: usize,
    pub fonts: usize,
    pub strikethrough: usize,
    pub underlined: usize,
    pub truncated: bool,
}

impl BedrockDrops {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.click_events == 0
            && self.hover_events == 0
            && self.fonts == 0
            && self.strikethrough == 0
            && self.underlined == 0
            && !self.truncated
    }
}

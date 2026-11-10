/// The features present in canonical Java text components that Bedrock Edition does not
/// support and were dropped during conversion.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct BedrockDrops {
    /// Number of `click_event` properties dropped.
    pub click_events: usize,
    /// Number of `hover_event` properties dropped.
    pub hover_events: usize,
    /// Number of custom font specifications dropped.
    pub fonts: usize,
    /// Number of strikethrough styles or formatting codes dropped.
    pub strikethrough: usize,
    /// Number of underlined styles or formatting codes dropped.
    pub underlined: usize,
    /// Whether the output stopped at the size bound and lost the rest of the text.
    pub truncated: bool,
}

impl BedrockDrops {
    /// Returns true if no features were dropped.
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

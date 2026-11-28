use mcrs_minecraft_text::DeclaredStyle;

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
    pub(crate) fn count(&mut self, declared: DeclaredStyle) {
        let tally = |count: &mut usize, present: bool| {
            if present {
                *count = count.saturating_add(1);
            }
        };
        tally(&mut self.click_events, declared.click_event);
        tally(&mut self.hover_events, declared.hover_event);
        tally(&mut self.fonts, declared.font);
        tally(&mut self.strikethrough, declared.strikethrough_enabled);
        tally(&mut self.underlined, declared.underlined_enabled);
    }

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

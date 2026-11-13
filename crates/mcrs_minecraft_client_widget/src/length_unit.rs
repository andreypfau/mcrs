/// The unit in which an edit box counts its length (EDIT-LEN-002).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LengthUnit {
    /// UTF-16 code units, as a vanilla server counts them.
    #[default]
    CodeUnits,
    /// Characters, as the input box of the chat counts them (CHAT-DEV-002).
    Characters,
}

impl LengthUnit {
    fn of(self, c: char) -> usize {
        match self {
            Self::CodeUnits => c.len_utf16(),
            Self::Characters => 1,
        }
    }

    pub(crate) fn length(self, text: &str) -> usize {
        text.chars().map(|c| self.of(c)).sum()
    }

    pub(crate) fn head(self, text: &str, room: usize) -> usize {
        let mut used = 0_usize;
        for (index, c) in text.char_indices() {
            used = used.saturating_add(self.of(c));
            if used > room {
                return index;
            }
        }
        text.len()
    }
}

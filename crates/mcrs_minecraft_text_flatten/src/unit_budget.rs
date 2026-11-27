pub const MAX_UNITS: usize = 1_048_576;

#[derive(Clone, Copy, Debug)]
pub struct UnitBudget {
    left: usize,
}

impl UnitBudget {
    #[must_use]
    pub const fn new() -> Self {
        Self { left: MAX_UNITS }
    }

    pub fn take<'a>(&mut self, run: &'a str) -> &'a str {
        let mut end = 0;
        for (at, character) in run.char_indices() {
            let Some(left) = self.left.checked_sub(character.len_utf16()) else {
                self.left = 0;
                break;
            };
            self.left = left;
            end = at + character.len_utf8();
        }
        run.get(..end).unwrap_or_default()
    }
}

impl Default for UnitBudget {
    fn default() -> Self {
        Self::new()
    }
}

use crate::entries::Entries;

pub const LIST_ROWS: usize = 10;

#[derive(Clone, Debug, PartialEq)]
pub struct SuggestionMenu {
    entries: Entries,
    origin: String,
    result: String,
    selected: usize,
    offset: usize,
    pub(crate) cycle: bool,
}

impl SuggestionMenu {
    pub(crate) fn new(entries: &Entries, origin: &str) -> Self {
        let mut menu = Self {
            entries: entries.clone(),
            origin: origin.to_owned(),
            result: String::new(),
            selected: 0,
            offset: 0,
            cycle: false,
        };
        menu.select(0);
        menu
    }

    pub fn entries(&self) -> &Entries {
        &self.entries
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn offset(&self) -> usize {
        self.offset
    }

    pub(crate) fn result(&self) -> &str {
        &self.result
    }

    pub(crate) fn result_caret(&self) -> usize {
        self.entries.range().start + self.entries.text(self.selected).map_or(0, str::len)
    }

    pub fn ghost<'s>(&'s self, input: &str) -> Option<&'s str> {
        self.result
            .strip_prefix(input)
            .filter(|rest| !rest.is_empty())
    }

    pub fn select(&mut self, index: isize) {
        let count = self.entries.len();
        let Some(selected) = isize::try_from(count)
            .ok()
            .filter(|&count| count > 0)
            .map(|count| index.rem_euclid(count).unsigned_abs())
        else {
            return;
        };
        self.selected = selected;
        if selected < self.offset {
            self.offset = selected;
        } else if selected > self.offset + LIST_ROWS - 1 {
            self.offset = selected + 1 - LIST_ROWS;
        }
        self.offset = self.offset.min(count.saturating_sub(LIST_ROWS));
        let range = self.entries.range();
        self.result.clear();
        self.result
            .push_str(self.origin.get(..range.start).unwrap_or_default());
        self.result
            .push_str(self.entries.text(selected).unwrap_or_default());
        self.result
            .push_str(self.origin.get(range.end..).unwrap_or_default());
    }

    pub fn step(&mut self, delta: isize) {
        let Ok(selected) = isize::try_from(self.selected) else {
            return;
        };
        self.select(selected.saturating_add(delta));
    }

    pub fn scroll(&mut self, step: isize) {
        let max = self.entries.len().saturating_sub(LIST_ROWS);
        self.offset = self.offset.saturating_add_signed(-step).min(max);
    }

    pub fn rows(&self) -> impl Iterator<Item = (usize, &str)> {
        (self.offset..self.entries.len().min(self.offset + LIST_ROWS))
            .map(|index| (index, self.entries.text(index).unwrap_or_default()))
    }
}

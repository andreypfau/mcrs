use crate::entries::Entries;

/// The most rows the suggestion list shows.
pub const LIST_ROWS: usize = 10;

/// The suggestion list while it shows.
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

    /// The suggestions the list shows.
    pub fn entries(&self) -> &Entries {
        &self.entries
    }

    /// The index of the selected entry.
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// The index of the entry in the top row.
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// The input with the range replaced by the selected suggestion.
    pub(crate) fn result(&self) -> &str {
        &self.result
    }

    /// The caret after an insert of the selected suggestion.
    pub(crate) fn result_caret(&self) -> usize {
        self.entries.range().start + self.entries.text(self.selected).map_or(0, str::len)
    }

    /// The rest of the result after the current input, when the result starts with it.
    pub fn ghost<'s>(&'s self, input: &str) -> Option<&'s str> {
        self.result
            .strip_prefix(input)
            .filter(|rest| !rest.is_empty())
    }

    /// Selects entry `index` modulo the count, and scrolls it into the rows.
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

    /// Selects the entry `delta` entries after the selected one, wrapping around.
    pub fn step(&mut self, delta: isize) {
        let Ok(selected) = isize::try_from(self.selected) else {
            return;
        };
        self.select(selected.saturating_add(delta));
    }

    /// A wheel step over the list, upwards positive; the selection stays.
    pub fn scroll(&mut self, step: isize) {
        let max = self.entries.len().saturating_sub(LIST_ROWS);
        self.offset = self.offset.saturating_add_signed(-step).min(max);
    }

    /// The entries in the rows, from the top row down.
    pub fn rows(&self) -> impl Iterator<Item = (usize, &str)> {
        (self.offset..self.entries.len().min(self.offset + LIST_ROWS))
            .map(|index| (index, self.entries.text(index).unwrap_or_default()))
    }
}

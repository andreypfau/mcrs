use crate::column_asks::{ColumnAnswer, ColumnAsk, ColumnHold};

#[derive(Debug, Default)]
pub struct ColumnStoreMail {
    pub(crate) holds: Vec<ColumnHold>,
    pub(crate) asks: Vec<ColumnAsk>,
    pub(crate) answers: Vec<ColumnAnswer>,
}

impl ColumnStoreMail {
    #[must_use]
    pub(crate) fn is_empty(&self) -> bool {
        self.holds.is_empty() && self.asks.is_empty() && self.answers.is_empty()
    }

    pub(crate) fn append(&mut self, other: &mut Self) {
        self.holds.append(&mut other.holds);
        self.asks.append(&mut other.asks);
        self.answers.append(&mut other.answers);
    }
}

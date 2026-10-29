/// A range of bytes of a text, `start ≤ end`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub const fn is_empty(self) -> bool {
        self.start == self.end
    }

    pub const fn contains(self, position: usize) -> bool {
        self.start <= position && position <= self.end
    }

    pub fn text(self, input: &str) -> &str {
        input.get(self.start..self.end).unwrap_or_default()
    }
}

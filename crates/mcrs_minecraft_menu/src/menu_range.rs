use core::ops::Range;

/// The order in which a [`MenuRange`] visits its menu indices.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MenuOrder {
    Ascending,
    Descending,
}

/// The menu indices `start..end`, visited in `order`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MenuRange {
    pub start: u16,
    pub end: u16,
    pub order: MenuOrder,
}

impl MenuRange {
    #[must_use]
    pub const fn new(start: u16, end: u16, order: MenuOrder) -> Self {
        Self { start, end, order }
    }

    #[must_use]
    pub fn iter(&self) -> MenuRangeIter {
        MenuRangeIter {
            indices: self.start..self.end,
            order: self.order,
        }
    }
}

impl IntoIterator for MenuRange {
    type Item = u16;
    type IntoIter = MenuRangeIter;

    fn into_iter(self) -> MenuRangeIter {
        self.iter()
    }
}

/// The menu indices of a [`MenuRange`], in its order.
#[derive(Clone, Debug)]
pub struct MenuRangeIter {
    indices: Range<u16>,
    order: MenuOrder,
}

impl Iterator for MenuRangeIter {
    type Item = u16;

    fn next(&mut self) -> Option<u16> {
        match self.order {
            MenuOrder::Ascending => self.indices.next(),
            MenuOrder::Descending => self.indices.next_back(),
        }
    }
}

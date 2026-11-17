use crate::EntryOutOfRange;

const OUTSIDE: i16 = -999;
const NOWHERE: i16 = -1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Clicked {
    Entry(u16),
    Outside,
    Nowhere,
}

impl From<i16> for Clicked {
    fn from(index: i16) -> Self {
        match u16::try_from(index) {
            Ok(entry) => Self::Entry(entry),
            Err(_) if index == OUTSIDE => Self::Outside,
            Err(_) => Self::Nowhere,
        }
    }
}

impl TryFrom<Clicked> for i16 {
    type Error = EntryOutOfRange;

    fn try_from(clicked: Clicked) -> Result<Self, Self::Error> {
        match clicked {
            Clicked::Entry(entry) => Self::try_from(entry).map_err(|_| EntryOutOfRange),
            Clicked::Outside => Ok(OUTSIDE),
            Clicked::Nowhere => Ok(NOWHERE),
        }
    }
}

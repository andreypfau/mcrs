use mcrs_minecraft_item_table::ItemTable;

use crate::smoke_table::SmokeTable;

#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct PredicateTables<'a> {
    pub smoke: &'a SmokeTable,
    pub items: &'a ItemTable,
}

impl<'a> PredicateTables<'a> {
    #[must_use]
    pub const fn new(smoke: &'a SmokeTable, items: &'a ItemTable) -> Self {
        Self { smoke, items }
    }
}

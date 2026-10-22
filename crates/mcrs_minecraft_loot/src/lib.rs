#[rustfmt::skip]
pub mod keys;

// chisle: loot conditions, item modifiers and slot sources are read for their names only; their
// values get types when a loot table is rolled.
pub mod condition {
    pub enum LootCondition {}
}

pub mod function {
    pub enum LootItemFunction {}
}

pub mod slot {
    pub enum SlotSource {}
}

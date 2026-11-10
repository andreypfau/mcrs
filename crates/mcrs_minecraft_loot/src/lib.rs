#[rustfmt::skip]
pub mod context_float_provider_keys;
#[rustfmt::skip]
pub mod context_float_provider_type_keys;
#[rustfmt::skip]
pub mod context_int_provider_keys;
#[rustfmt::skip]
pub mod context_int_provider_type_keys;
#[rustfmt::skip]
pub mod context_key_set_keys;
#[rustfmt::skip]
pub mod loot_condition_type_keys;
#[rustfmt::skip]
pub mod loot_function_type_keys;
#[rustfmt::skip]
pub mod loot_nbt_provider_type_keys;
#[rustfmt::skip]
pub mod loot_pool_entry_type_keys;
#[rustfmt::skip]
pub mod loot_score_provider_type_keys;
#[rustfmt::skip]
pub mod loot_table_keys;
#[rustfmt::skip]
pub mod predicate_keys;
#[rustfmt::skip]
pub mod registry_keys;
#[rustfmt::skip]
pub mod slot_source_type_keys;

pub mod loot;

pub mod condition {
    pub enum LootCondition {}
}

pub mod function {
    pub enum LootItemFunction {}
}

pub mod slot {
    pub enum SlotSource {}
}

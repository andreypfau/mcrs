// The `Sync` check of a loot table walks text through `Holder`, whose `V::Registry` projection
// keeps that cycle from being coinductive, so it goes deeper than the default limit.
#![recursion_limit = "256"]
#[rustfmt::skip]
pub mod keys;
mod buffer;
pub mod condition;
pub mod entry;
pub mod function;
pub mod number;
pub mod provider;
pub mod slot;
pub mod table;

pub use condition::{EntityTarget, LootCondition};
pub use entry::LootPoolEntry;
pub use function::LootItemFunction;
pub use number::{FloatExpression, IntExpression};
pub use slot::SlotSource;
pub use table::{LootPool, LootTableBody, LootTableFile};

mcrs_minecraft_registries::read_with_serde!(
    crate::condition::LootCondition,
    crate::function::LootItemFunction,
    crate::slot::SlotSource,
    crate::LootTableFile,
    crate::number::IntExpression,
    crate::number::FloatExpression
);

mcrs_minecraft_registries::declare_registries!(
    crate::condition::LootCondition => crate::keys::PREDICATE, Reloadable;
    crate::function::LootItemFunction => crate::keys::ITEM_MODIFIER, Reloadable;
    crate::slot::SlotSource => crate::keys::SLOT_SOURCE, Reloadable;
    crate::LootTableFile => mcrs_minecraft_item::keys::LOOT_TABLE, Reloadable;
    crate::number::IntExpression => mcrs_minecraft_item::keys::CONTEXT_INT_PROVIDER, Reloadable;
    crate::number::FloatExpression => mcrs_minecraft_item::keys::CONTEXT_FLOAT_PROVIDER, Reloadable;
);

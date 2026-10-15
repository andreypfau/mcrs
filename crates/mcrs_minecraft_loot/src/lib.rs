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

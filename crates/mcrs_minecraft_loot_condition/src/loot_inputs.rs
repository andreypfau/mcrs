use crate::loot_context::LootContext;
use crate::named_entries::NamedEntries;

/// What one test of a condition or one read of a provider borrows: the named entries that the
/// bake resolved the keys against, the context and the world.
#[derive(Debug)]
pub struct LootInputs<'i, 'c, E, W> {
    pub named: &'i NamedEntries,
    pub context: &'i LootContext<'c, E>,
    pub world: &'i W,
}

impl<E, W> Clone for LootInputs<'_, '_, E, W> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<E, W> Copy for LootInputs<'_, '_, E, W> {}

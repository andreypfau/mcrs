use glam::DVec3;
use mcrs_minecraft_loot::context_key_set_keys::ContextKeySet;
use mcrs_voxel::BlockStateId;

use crate::tool::Tool;

/// The parameters that one evaluation reads (LOOT-CTX-010), and the key set of the source that
/// made the context (LOOT-CTX-012). `None` is an absent parameter. `E` is the entity handle of
/// the host.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct LootContext<'a, E> {
    pub origin: Option<DVec3>,
    pub this_entity: Option<E>,
    pub tool: Option<Tool<'a>>,
    pub block_state: Option<BlockStateId>,
    pub explosion_radius: Option<f32>,
    key_set: ContextKeySet,
}

impl<E> LootContext<'_, E> {
    /// A context of the key set `key_set` without parameters; the source sets them
    /// (LOOT-CTX-012). `None` for `generic`, which is the type of a table and never the key set
    /// of a source (LOOT-CTX-013).
    #[must_use]
    pub const fn try_new(key_set: ContextKeySet) -> Option<Self> {
        if matches!(key_set, ContextKeySet::Generic) {
            return None;
        }
        Some(Self {
            origin: None,
            this_entity: None,
            tool: None,
            block_state: None,
            explosion_radius: None,
            key_set,
        })
    }

    /// The key set of the source; a roll checks the parameters of the context against it.
    #[must_use]
    pub const fn key_set(&self) -> ContextKeySet {
        self.key_set
    }
}

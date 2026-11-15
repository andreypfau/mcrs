use glam::DVec3;
use mcrs_voxel::BlockStateId;

use crate::tool::Tool;

/// The parameters that one evaluation reads (LOOT-CTX-010). `None` is an absent
/// parameter. `E` is the entity handle of the host.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct LootContext<'a, E> {
    pub origin: Option<DVec3>,
    pub this_entity: Option<E>,
    pub tool: Option<Tool<'a>>,
    pub block_state: Option<BlockStateId>,
    pub explosion_radius: Option<f32>,
}

impl<E> LootContext<'_, E> {
    /// A context without parameters.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            origin: None,
            this_entity: None,
            tool: None,
            block_state: None,
            explosion_radius: None,
        }
    }
}

impl<E> Default for LootContext<'_, E> {
    fn default() -> Self {
        Self::new()
    }
}

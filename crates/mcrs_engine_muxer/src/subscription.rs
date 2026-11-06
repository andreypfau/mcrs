use std::ops::Range;

use crate::tier::Tier;

/// The number of layers of a tile.
pub(crate) const LAYERS: usize = 3;

/// The first rank after each layer: layer 0 holds the first 32 ranks, layer 1 the next 64,
/// layer 2 the rest.
const LAYER_ENDS: [usize; LAYERS] = [32, 96, usize::MAX];

/// The ranks of the subjects of a tile in `layer`. The rank of a subject is its position in the
/// tile in the order of [`EntityId`](crate::EntityId), the same for all viewers.
pub(crate) fn layer_ranks(layer: usize) -> Range<usize> {
    let start = layer
        .checked_sub(1)
        .and_then(|before| LAYER_ENDS.get(before))
        .copied()
        .unwrap_or(0);
    let end = LAYER_ENDS.get(layer).copied().unwrap_or(start);
    start..end
}

/// What a viewer receives of one layer of a tile.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct LayerState {
    /// The tier of the layer, or `None` when the cap leaves the layer out.
    pub(crate) tier: Option<Tier>,
    /// The tiers that the budget took off the layer.
    pub(crate) lowered: u8,
    /// The priority that the layer gathered while the budget kept it lowered.
    pub(crate) waited: u32,
}

/// What a viewer receives of one tile. The admitted layers are always a prefix.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Subscription {
    /// The tier of the tile by its distance, with hysteresis.
    pub(crate) reach: Tier,
    pub(crate) layers: [LayerState; LAYERS],
}

impl Subscription {
    /// The tier of the subject with `rank`, or `None` when the viewer does not receive it.
    pub(crate) fn tier_of(&self, rank: usize) -> Option<Tier> {
        let layer = LAYER_ENDS.iter().position(|&end| rank < end)?;
        self.layers.get(layer)?.tier
    }

    /// The number of ranks that the viewer receives, from rank 0.
    pub(crate) fn visible_ranks(&self) -> usize {
        let admitted = self
            .layers
            .iter()
            .take_while(|layer| layer.tier.is_some())
            .count();
        admitted
            .checked_sub(1)
            .map_or(0, |last| layer_ranks(last).end)
    }
}

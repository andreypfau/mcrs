use std::ops::Range;

use crate::tier::Tier;

pub(crate) const LAYERS: usize = 3;

const LAYER_ENDS: [usize; LAYERS] = [32, 96, usize::MAX];

pub(crate) fn layer_ranks(layer: usize) -> Range<usize> {
    let start = layer
        .checked_sub(1)
        .and_then(|before| LAYER_ENDS.get(before))
        .copied()
        .unwrap_or(0);
    let end = LAYER_ENDS.get(layer).copied().unwrap_or(start);
    start..end
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct LayerState {
    pub(crate) tier: Option<Tier>,
    pub(crate) lowered: u8,
    pub(crate) waited: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Subscription {
    pub(crate) reach: Tier,
    pub(crate) layers: [LayerState; LAYERS],
}

impl Subscription {
    pub(crate) fn tier_of(&self, rank: usize) -> Option<Tier> {
        let layer = LAYER_ENDS.iter().position(|&end| rank < end)?;
        self.layers.get(layer)?.tier
    }

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

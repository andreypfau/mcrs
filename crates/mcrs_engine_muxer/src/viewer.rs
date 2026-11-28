use std::collections::{BTreeMap, BTreeSet};

use bytes::Bytes;

use crate::planner::{Measured, Point};
use crate::subscription::Subscription;
use crate::tier::Tier;
use crate::tile_blob::TileBlob;
use crate::tile_reach::TileReach;
use crate::tile_stream::TileStream;
use crate::view::View;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_key::TileKey;

#[derive(Debug)]
pub(crate) struct Viewer {
    pub(crate) view: View,
    pub(crate) subscriptions: BTreeMap<TileKey, Subscription>,
    pub(crate) held: BTreeSet<EntityId>,
    pub(crate) chunks: Vec<Bytes>,
    pub(crate) counted: usize,
    average_sixteenths: u64,
    pub(crate) measured: Measured,
    pub(crate) settled_at: Option<Point>,
    pub(crate) region: BTreeMap<TileKey, TileReach>,
    pub(crate) tracked_at: Option<Point>,
}

impl Viewer {
    pub(crate) fn new(view: View) -> Self {
        Self {
            view,
            subscriptions: BTreeMap::new(),
            held: BTreeSet::new(),
            chunks: Vec::new(),
            counted: 0,
            average_sixteenths: 0,
            measured: Measured::default(),
            settled_at: None,
            region: BTreeMap::new(),
            tracked_at: None,
        }
    }

    pub(crate) fn append_tier(
        &mut self,
        tile: TileKey,
        stream: &TileStream,
        tier: Tier,
        blob: &TileBlob,
        gained: &[Gain],
    ) {
        let Some(subscription) = self.subscriptions.get(&tile) else {
            return;
        };
        let own = self.view.subject;
        for (layer, layer_state) in subscription.layers.iter().enumerate() {
            let window = stream
                .window(layer)
                .filter(|_| layer_state.tier == Some(tier));
            let Some(window) = window else {
                continue;
            };
            blob.append_kept(&mut self.chunks, |subject| {
                window.contains(&subject)
                    && Some(subject) != own
                    && !gained.iter().any(|gain| gain.subject == subject)
            });
        }
    }

    pub(crate) fn measure(&mut self, budget: u64) {
        let new = self.chunks.get(self.counted..).unwrap_or_default();
        let bytes: u64 = new.iter().map(|chunk| chunk.len() as u64).sum();
        self.counted = self.chunks.len();
        self.average_sixteenths = self
            .average_sixteenths
            .saturating_sub(self.average_sixteenths / 16)
            .saturating_add(bytes);
        let average = self.average_sixteenths / 16;
        let previous = self.measured;
        self.measured = Measured {
            average,
            over: if average > budget {
                previous.over.saturating_add(1)
            } else {
                0
            },
            calm: if average.saturating_mul(10) < budget.saturating_mul(7) {
                previous.calm.saturating_add(1)
            } else {
                0
            },
        };
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Gain {
    pub(crate) subject: EntityId,
    pub(crate) tier: Tier,
}

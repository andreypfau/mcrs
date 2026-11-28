use std::collections::{BTreeMap, BTreeSet};
use std::mem;
use std::num::NonZeroU32;
use std::ops::RangeInclusive;

use bytes::Bytes;

use crate::entity_id::ViewerId;
use crate::place::Place;
use crate::planner;
use crate::planner::{Measured, Point, SubscriptionChange, TileHeader};
use crate::replication_codec::{Hold, ReplicationCodec};
use crate::subscription::Subscription;
use crate::tier::Tier;
use crate::tile_blob::TileBlob;
use crate::tile_reach;
use crate::tile_reach::TileReach;
use crate::tile_stream::TileStream;
use crate::view::View;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_key::TileKey;

#[derive(Debug)]
pub(crate) struct Viewer {
    view: View,
    subscriptions: BTreeMap<TileKey, Subscription>,
    held: BTreeSet<EntityId>,
    chunks: Vec<Bytes>,
    counted: usize,
    average_sixteenths: u64,
    measured: Measured,
    settled_at: Option<Point>,
    region: BTreeMap<TileKey, TileReach>,
    tracked_at: Option<Point>,
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

    pub(crate) fn retarget(&mut self, view: View) {
        self.view = view;
        self.settled_at = None;
    }

    pub(crate) fn dropped_tiles(&self, view: &View) -> Vec<TileKey> {
        let restart = self.view.subject != view.subject;
        self.subscriptions
            .keys()
            .filter(|&&tile| restart || !view.contains(tile))
            .copied()
            .collect()
    }

    pub(crate) fn subscribed_tiles(&self) -> impl Iterator<Item = TileKey> + '_ {
        self.subscriptions.keys().copied()
    }

    pub(crate) fn watched_tiles(&self) -> impl Iterator<Item = TileKey> + '_ {
        self.region.keys().copied()
    }

    pub(crate) fn into_chunks(self) -> Vec<Bytes> {
        self.chunks
    }

    pub(crate) fn take_chunks(&mut self) -> Option<Vec<Bytes>> {
        if self.chunks.is_empty() {
            return None;
        }
        self.counted = 0;
        Some(mem::take(&mut self.chunks))
    }

    pub(crate) fn receive(&mut self, chunks: &mut Vec<Bytes>) {
        self.chunks.append(chunks);
    }

    pub(crate) fn hold<C: ReplicationCodec>(
        &mut self,
        codec: &mut C,
        viewer: ViewerId,
        change: Hold<'_>,
    ) {
        codec.hold(viewer, change, &mut self.chunks);
    }

    pub(crate) fn position<C: ReplicationCodec>(&self, codec: &C, tile_width: NonZeroU32) -> Point {
        let width = f64::from(tile_width.get());
        match self
            .view
            .subject
            .and_then(|subject| codec.position(subject))
        {
            Some([x, _, z]) => Point { x, z },
            None => Point {
                x: (f64::from(self.view.center.x) + 0.5) * width,
                z: (f64::from(self.view.center.z) + 0.5) * width,
            },
        }
    }

    pub(crate) fn is_settled(
        &self,
        position: Point,
        mut changed: impl Iterator<Item = TileKey>,
    ) -> bool {
        self.settled_at == Some(position)
            && !planner::reads_budget(self.measured)
            && !changed.any(|tile| self.view.contains(tile))
    }

    pub(crate) fn moved_little(&self, position: Point) -> bool {
        self.tracked_at.is_some_and(|at| {
            (at.x - position.x).hypot(at.z - position.z) < tile_reach::RETRACK_STEP
        })
    }

    pub(crate) fn retrack(
        &mut self,
        position: Point,
        tile_width: NonZeroU32,
        range: u32,
    ) -> BTreeMap<TileKey, TileReach> {
        let width = f64::from(tile_width.get());
        let region = tile_reach::region(position, &self.view, width, range);
        self.tracked_at = Some(position);
        mem::replace(&mut self.region, region)
    }

    pub(crate) fn reach(&self, tile: TileKey) -> Option<TileReach> {
        self.region.get(&tile).copied()
    }

    pub(crate) fn reaches(&self) -> impl Iterator<Item = (TileKey, TileReach)> + '_ {
        self.region.iter().map(|(&tile, &reach)| (tile, reach))
    }

    pub(crate) fn wants<C: ReplicationCodec>(
        &self,
        codec: &C,
        subject: EntityId,
        place: Option<Place>,
    ) -> bool {
        let Some(place) = place else {
            return false;
        };
        if Some(subject) == self.view.subject {
            return false;
        }
        let held = self.held.contains(&subject);
        place.tracks(held, || {
            let viewer = self.tracked_at?;
            let [x, _, z] = codec.position(subject)?;
            Some((x - viewer.x).hypot(z - viewer.z))
        })
    }

    pub(crate) fn wanted_tier(
        &self,
        tile: TileKey,
        rank: usize,
        subject: EntityId,
    ) -> Option<Tier> {
        if Some(subject) == self.view.subject {
            return None;
        }
        self.subscriptions.get(&tile)?.tier_of(rank)
    }

    pub(crate) fn visible_ranks(&self, tile: TileKey) -> usize {
        self.subscriptions
            .get(&tile)
            .map_or(0, Subscription::visible_ranks)
    }

    pub(crate) fn receives(&self, tile: TileKey, tier: Tier) -> bool {
        self.subscriptions.get(&tile).is_some_and(|subscription| {
            subscription
                .layers
                .iter()
                .any(|layer| layer.tier == Some(tier))
        })
    }

    pub(crate) fn subscription(&self, tile: TileKey) -> Subscription {
        self.subscriptions.get(&tile).copied().unwrap_or_default()
    }

    pub(crate) fn set_subscription(&mut self, tile: TileKey, subscription: Option<Subscription>) {
        match subscription {
            Some(subscription) => {
                self.subscriptions.insert(tile, subscription);
            }
            None => {
                self.subscriptions.remove(&tile);
            }
        }
    }

    pub(crate) fn settle_hold(
        &mut self,
        subject: EntityId,
        wanted: Option<Tier>,
    ) -> Option<HoldChange> {
        let held = self.held.contains(&subject);
        match wanted {
            Some(tier) if !held => {
                self.held.insert(subject);
                Some(HoldChange::Gained(Gain { subject, tier }))
            }
            None if held => {
                self.held.remove(&subject);
                Some(HoldChange::Lost(subject))
            }
            _ => None,
        }
    }

    pub(crate) fn gain<C: ReplicationCodec>(
        &mut self,
        codec: &mut C,
        viewer: ViewerId,
        members: &[EntityId],
        tier: Tier,
    ) {
        let own = self.view.subject;
        for &subject in members {
            if Some(subject) != own && self.held.insert(subject) {
                codec.hold(viewer, Hold::Gain { subject, tier }, &mut self.chunks);
            }
        }
    }

    pub(crate) fn lose(&mut self, members: &[EntityId], lost: &mut Vec<EntityId>) {
        lost.extend(members.iter().filter(|subject| self.held.remove(subject)));
    }

    pub(crate) fn append_keyframe(&mut self, blob: &TileBlob, window: &RangeInclusive<EntityId>) {
        let own = self.view.subject;
        blob.append_kept(&mut self.chunks, |subject| {
            window.contains(&subject) && Some(subject) != own
        });
    }

    pub(crate) fn plan(
        &mut self,
        tile_width: NonZeroU32,
        position: Point,
        headers: &mut Vec<TileHeader>,
        count: impl Fn(TileKey) -> usize,
    ) -> Vec<SubscriptionChange> {
        headers.clear();
        headers.extend(self.view.tiles().map(|tile| TileHeader {
            tile,
            count: count(tile),
        }));
        let changes = planner::plan(
            tile_width,
            position,
            &self.subscriptions,
            headers,
            self.measured,
        );
        self.settled_at = changes.is_empty().then_some(position);
        changes
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

#[derive(Clone, Copy, Debug)]
pub(crate) enum HoldChange {
    Gained(Gain),
    Lost(EntityId),
}

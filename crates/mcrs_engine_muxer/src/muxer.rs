use std::collections::{BTreeMap, BTreeSet};
use std::mem;
use std::num::NonZeroU32;

use crate::entity_event::{EntityEvent, SubjectEvent};
use crate::entity_id::{EntityId, ViewerId};
use crate::planner;
use crate::planner::{BUDGET, Point, SubscriptionChange, TileHeader};
use crate::replication_codec::{BlobKind, Hold, ReplicationCodec};
use crate::subscription::{LAYERS, Subscription, layer_ranks};
use crate::tier::Tier;
use crate::tile_blob::TileBlob;
use crate::tile_delta::{TileDelta, TileDeltas};
use crate::tile_key::TileKey;
use crate::tile_reach;
use crate::tile_reach::{MAX_RANGE, TileReach};
use crate::tile_stream::TileStream;
use crate::view::View;
use crate::viewer::{Gain, Viewer};
use crate::viewer_bytes::ViewerBytes;

/// Decides which entity bytes each viewer receives. It starts no thread and reads no clock, so
/// a game can call it from its own loop.
///
/// Each tick, it plans each viewer: the tiles near the viewer get a fast tier, the far ones a
/// slow tier, the cap K limits the subjects that the viewer holds, and the budget lowers tiers
/// when the viewer receives too many bytes. It encodes each tile once for each tier that is due
/// and has viewers, and the viewers of a tier share the bytes, cut by layer.
///
/// A subject with a tracking range ([`ReplicationCodec::tracking_range`]) has none of this: a
/// viewer holds it while it is within its range, the codec sends its updates, and it counts
/// neither for the cap nor for the budget. The muxer judges such a subject again only when it
/// enters or leaves a tile, when it moves in a tile on the edge of the range of a viewer, or when
/// the viewer moves.
#[derive(Debug)]
pub struct Muxer<C> {
    codec: C,
    /// The width of a tile on X and Z, in the units of the game. The distance of a tile to a
    /// viewer is the distance on X and Z from the viewer to the nearest point of the tile.
    tile_width: NonZeroU32,
    tiles: BTreeMap<TileKey, TileStream>,
    /// The tile of each subject.
    homes: BTreeMap<EntityId, TileKey>,
    viewers: BTreeMap<ViewerId, Viewer>,
    /// The bytes of viewers that no longer subscribe.
    loose: Vec<ViewerBytes>,
    owed: Vec<ViewerBytes>,
    /// The tiles with events that some tier has not encoded yet.
    unsent: BTreeSet<TileKey>,
    /// The keyframes of this tick, shared by the viewers that change tier in it.
    keyframes: BTreeMap<TileTier, TileBlob>,
    headers: Vec<TileHeader>,
    candidates: Vec<EntityId>,
    scratch_viewers: Vec<ViewerId>,
    scratch_tiles: Vec<TileKey>,
    tick: u64,
    /// The tracking range of each ranged subject.
    ranges: BTreeMap<EntityId, u32>,
    /// The smallest and the largest tracking range seen, or `None` before the first.
    range_bounds: Option<RangeBounds>,
    /// The tiles of this tick with events of ranged subjects.
    touches: Vec<TileTouch>,
    /// The ranged subjects that despawned in this tick.
    departed: Vec<EntityId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RangeBounds {
    min: u32,
    max: u32,
}

/// The ranged subjects of one tile delta that a viewer of the tile may have to judge again.
#[derive(Debug)]
struct TileTouch {
    tile: TileKey,
    /// The subjects that spawned, despawned, came in or went out.
    crossed: Vec<EntityId>,
    /// The subjects that moved inside the tile.
    moved: Vec<EntityId>,
}

/// Which tiles of the region of a viewer the muxer judges again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Retrack {
    /// Each tile, also when the viewer did not move.
    All,
    /// Only after the viewer moved, and then only the tiles that were not and are not inside
    /// every range.
    Moved,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct TileTier {
    tile: TileKey,
    tier: Tier,
}

/// A tile whose subjects changed in a tick.
struct Membership {
    tile: TileKey,
    before: Vec<EntityId>,
    /// The subjects that came in or went out.
    crossed: Vec<EntityId>,
}

/// What the clients gain and lose in a tick, before the muxer sends it.
#[derive(Default)]
struct Settled {
    gained: BTreeMap<ViewerId, Vec<Gain>>,
    lost: BTreeMap<ViewerId, Vec<EntityId>>,
}

impl<C: ReplicationCodec> Muxer<C> {
    #[must_use]
    pub fn new(codec: C, tile_width: NonZeroU32) -> Self {
        Self {
            codec,
            tile_width,
            tiles: BTreeMap::new(),
            homes: BTreeMap::new(),
            viewers: BTreeMap::new(),
            loose: Vec::new(),
            owed: Vec::new(),
            unsent: BTreeSet::new(),
            keyframes: BTreeMap::new(),
            headers: Vec::new(),
            candidates: Vec::new(),
            scratch_viewers: Vec::new(),
            scratch_tiles: Vec::new(),
            tick: 0,
            ranges: BTreeMap::new(),
            range_bounds: None,
            touches: Vec::new(),
            departed: Vec::new(),
        }
    }

    #[cfg(test)]
    fn codec(&self) -> &C {
        &self.codec
    }

    pub fn codec_mut(&mut self) -> &mut C {
        &mut self.codec
    }

    /// Takes in the changes of one tick of a world, sends the moves of the tiers that are due,
    /// and plans each viewer for the next tick.
    pub fn publish(&mut self, deltas: &TileDeltas<C::Data>) {
        self.tick = deltas.tick;
        self.keyframes.clear();
        let bounds = self.range_bounds;
        let mut memberships = Vec::new();
        for delta in &deltas.tiles {
            self.codec.apply(deltas.tick, delta);
            memberships.extend(self.take_in(delta));
        }
        let mut settled = Settled::default();
        for membership in &memberships {
            self.settle(membership, &mut settled);
        }
        self.send_moves(&settled);
        self.send_settled(settled);

        for state in self.viewers.values_mut() {
            state.measure(BUDGET);
        }
        let retrack = if self.range_bounds == bounds {
            Retrack::Moved
        } else {
            Retrack::All
        };
        self.track(retrack);
        let mut viewers = mem::take(&mut self.scratch_viewers);
        viewers.clear();
        viewers.extend(
            self.viewers
                .iter()
                .filter(|(_, state)| !self.is_settled(state, &memberships))
                .map(|(&viewer, _)| viewer),
        );
        for &viewer in &viewers {
            self.replan(viewer);
        }
        self.scratch_viewers = viewers;
        for membership in &memberships {
            self.forget_if_empty(membership.tile);
        }

        let mut owed = mem::take(&mut self.owed);
        self.codec.finish_tick(deltas.tick, &mut owed);
        for ViewerBytes { viewer, mut chunks } in owed.drain(..) {
            match self.viewers.get_mut(&viewer) {
                Some(state) => state.chunks.append(&mut chunks),
                None => self.loose.push(ViewerBytes { viewer, chunks }),
            }
        }
        self.owed = owed;
    }

    /// Subscribes `viewer` to the tiles of `view` and plans it at once. A viewer that keeps its
    /// own subject keeps the tiles that stay in its view; a new own subject starts the view
    /// again.
    pub fn subscribe(&mut self, viewer: ViewerId, view: View) {
        let mut lost = Vec::new();
        if let Some(state) = self.viewers.get(&viewer) {
            let restart = state.view.subject != view.subject;
            let dropped: Vec<TileKey> = state
                .subscriptions
                .keys()
                .filter(|&&tile| restart || !view.contains(tile))
                .copied()
                .collect();
            for tile in dropped {
                let change = SubscriptionChange {
                    tile,
                    subscription: None,
                };
                self.resubscribe(viewer, change, &mut lost);
            }
        }
        self.viewers
            .entry(viewer)
            .or_insert_with(|| Viewer::new(view))
            .view = view;
        self.send_removal(viewer, &lost);
        self.replan(viewer);
        let mut settled = Settled::default();
        self.retrack(viewer, Retrack::All, &mut settled);
        self.send_settled(settled);
    }

    /// Forgets `viewer`, and so does the codec. It gets no removal, because its connection is
    /// gone or its client drops its entities by itself. The bytes it has not taken stay for
    /// [`Self::take_output`], and no bytes come after them.
    pub fn unsubscribe(&mut self, viewer: ViewerId) {
        let Some(state) = self.viewers.remove(&viewer) else {
            return;
        };
        self.codec.hold(viewer, Hold::Forget, &mut Vec::new());
        for tile in state.subscriptions.keys() {
            if let Some(stream) = self.tiles.get_mut(tile) {
                stream.viewers.remove(&viewer);
            }
            self.forget_if_empty(*tile);
        }
        for tile in state.region.keys() {
            if let Some(stream) = self.tiles.get_mut(tile) {
                stream.watchers.remove(&viewer);
            }
            self.forget_if_empty(*tile);
        }
        if !state.chunks.is_empty() {
            self.loose.push(ViewerBytes {
                viewer,
                chunks: state.chunks,
            });
        }
    }

    /// Hands out the bytes of each viewer that has some: first the bytes of viewers while they
    /// did not subscribe, then the viewers that subscribe in the order of [`ViewerId`]. Append
    /// the outputs to the connections in this order: the bytes of a viewer before it subscribed
    /// come before the bytes of its subscription.
    pub fn take_output(&mut self) -> impl Iterator<Item = ViewerBytes> + '_ {
        let Self { viewers, loose, .. } = self;
        loose
            .drain(..)
            .chain(viewers.iter_mut().filter_map(|(&viewer, state)| {
                if state.chunks.is_empty() {
                    return None;
                }
                state.counted = 0;
                Some(ViewerBytes {
                    viewer,
                    chunks: mem::take(&mut state.chunks),
                })
            }))
    }

    /// Moves the subjects that come into or go out of the tile of `delta`. Returns the change of
    /// the subjects that the planner places, and records the events of the ranged subjects in
    /// `touches`.
    fn take_in(&mut self, delta: &TileDelta<C::Data>) -> Option<Membership> {
        let tile = delta.tile;
        let stream = self.tiles.entry(tile).or_default();
        let mut before: Option<Vec<EntityId>> = None;
        let mut crossed = Vec::new();
        let mut touch = TileTouch {
            tile,
            crossed: Vec::new(),
            moved: Vec::new(),
        };
        let mut planned_events = false;
        for SubjectEvent { subject, event } in &delta.events {
            let subject = *subject;
            let range = match event {
                EntityEvent::Spawn(_) | EntityEvent::Enter { .. } => {
                    self.ranges.get(&subject).copied().or_else(|| {
                        self.codec
                            .tracking_range(subject)
                            .map(|range| range.min(MAX_RANGE))
                    })
                }
                EntityEvent::Leave { .. } | EntityEvent::Despawn => stream
                    .range_of(subject)
                    .or_else(|| self.ranges.get(&subject).copied()),
                EntityEvent::Move(_) | EntityEvent::Data(_) => stream.range_of(subject),
            };
            if let Some(range) = range {
                match event {
                    EntityEvent::Spawn(_) | EntityEvent::Enter { .. } => {
                        stream.insert_ranged(subject, range);
                        self.homes.insert(subject, tile);
                        self.ranges.insert(subject, range);
                        self.range_bounds = Some(match self.range_bounds {
                            Some(bounds) => RangeBounds {
                                min: bounds.min.min(range),
                                max: bounds.max.max(range),
                            },
                            None => RangeBounds {
                                min: range,
                                max: range,
                            },
                        });
                        touch.crossed.push(subject);
                    }
                    EntityEvent::Leave { .. } | EntityEvent::Despawn => {
                        stream.remove_ranged(subject);
                        if self.homes.get(&subject) == Some(&tile) {
                            self.homes.remove(&subject);
                        }
                        if matches!(event, EntityEvent::Despawn) {
                            self.departed.push(subject);
                        }
                        touch.crossed.push(subject);
                    }
                    EntityEvent::Move(_) => touch.moved.push(subject),
                    EntityEvent::Data(_) => {}
                }
                continue;
            }
            planned_events = true;
            match event {
                EntityEvent::Spawn(_) | EntityEvent::Enter { .. } => {
                    before.get_or_insert_with(|| stream.subjects.clone());
                    stream.insert(subject);
                    self.homes.insert(subject, tile);
                }
                EntityEvent::Leave { .. } | EntityEvent::Despawn => {
                    before.get_or_insert_with(|| stream.subjects.clone());
                    stream.remove(subject);
                    if self.homes.get(&subject) == Some(&tile) {
                        self.homes.remove(&subject);
                    }
                }
                EntityEvent::Move(_) | EntityEvent::Data(_) => continue,
            }
            crossed.push(subject);
        }
        if planned_events {
            stream.unsent = [true; Tier::ALL.len()];
            self.unsent.insert(tile);
        }
        if !touch.crossed.is_empty() || !touch.moved.is_empty() {
            self.touches.push(touch);
        }
        crossed.sort_unstable();
        crossed.dedup();
        Some(Membership {
            tile,
            before: before?,
            crossed,
        })
    }

    /// Judges the ranged subjects again: first for each viewer that moved, then for the subjects
    /// with events in this tick. Sends the spawns and the removals that follow.
    fn track(&mut self, retrack: Retrack) {
        let mut settled = Settled::default();
        let mut viewers = mem::take(&mut self.scratch_viewers);
        viewers.clear();
        viewers.extend(self.viewers.keys().copied());
        for &viewer in &viewers {
            self.retrack(viewer, retrack, &mut settled);
        }
        self.scratch_viewers = viewers;

        let mut touches = mem::take(&mut self.touches);
        let bounds = self.range_bounds;
        for touch in &touches {
            let Some(stream) = self.tiles.get(&touch.tile) else {
                continue;
            };
            for &viewer in &stream.watchers {
                let Some(state) = self.viewers.get_mut(&viewer) else {
                    continue;
                };
                let Some(&reach) = state.region.get(&touch.tile) else {
                    continue;
                };
                for &subject in &touch.crossed {
                    let place = self.homes.get(&subject).and_then(|home| {
                        Some(Place {
                            reach: *state.region.get(home)?,
                            range: *self.ranges.get(&subject)?,
                        })
                    });
                    let wanted = wants(&self.codec, state, subject, place);
                    record_hold(state, viewer, subject, wanted, &mut settled);
                }
                let on_edge = bounds.is_some_and(|bounds| {
                    reach.can_reach(bounds.max) && !reach.is_inside(bounds.min)
                });
                if !on_edge {
                    continue;
                }
                for &subject in &touch.moved {
                    let place = stream.range_of(subject).map(|range| Place { reach, range });
                    let wanted = wants(&self.codec, state, subject, place);
                    record_hold(state, viewer, subject, wanted, &mut settled);
                }
            }
        }
        for touch in touches.drain(..) {
            self.forget_if_empty(touch.tile);
        }
        self.touches = touches;
        for subject in mem::take(&mut self.departed) {
            if !self.homes.contains_key(&subject) {
                self.ranges.remove(&subject);
            }
        }
        self.send_settled(settled);
    }

    /// Measures the tracking region of `viewer` again from its position, and judges the ranged
    /// subjects of the tiles that `retrack` names.
    fn retrack(&mut self, viewer: ViewerId, retrack: Retrack, settled: &mut Settled) {
        let Some(state) = self.viewers.get(&viewer) else {
            return;
        };
        let position = self.position(&state.view);
        if retrack == Retrack::Moved && state.tracked_at == Some(position) {
            return;
        }
        let bounds = self.range_bounds;
        let width = f64::from(self.tile_width.get());
        let max = bounds.map_or(0, |bounds| bounds.max);
        let region = tile_reach::region(position, &state.view, width, max);
        let Some(state) = self.viewers.get_mut(&viewer) else {
            return;
        };
        let old = mem::replace(&mut state.region, region);
        state.tracked_at = Some(position);
        let inside = |reach: Option<&TileReach>| {
            bounds.is_some_and(|bounds| reach.is_some_and(|reach| reach.is_inside(bounds.min)))
        };
        let mut judged = mem::take(&mut self.scratch_tiles);
        judged.clear();
        for (&tile, reach) in &state.region {
            let was = old.get(&tile);
            if was.is_none() {
                self.tiles.entry(tile).or_default().watchers.insert(viewer);
            }
            let stays_inside = retrack == Retrack::Moved && inside(was) && inside(Some(reach));
            if !stays_inside {
                judged.push(tile);
            }
        }
        let first_dropped = judged.len();
        for &tile in old.keys() {
            if !state.region.contains_key(&tile) {
                if let Some(stream) = self.tiles.get_mut(&tile) {
                    stream.watchers.remove(&viewer);
                }
                judged.push(tile);
            }
        }
        for &tile in &judged {
            let Some(stream) = self.tiles.get(&tile) else {
                continue;
            };
            let reach = state.region.get(&tile).copied();
            for entry in &stream.ranged {
                let place = reach.map(|reach| Place {
                    reach,
                    range: entry.range,
                });
                let wanted = wants(&self.codec, state, entry.subject, place);
                record_hold(state, viewer, entry.subject, wanted, settled);
            }
        }
        for &tile in judged.get(first_dropped..).unwrap_or_default() {
            if self.tiles.get(&tile).is_some_and(TileStream::is_empty) {
                self.tiles.remove(&tile);
            }
        }
        self.scratch_tiles = judged;
    }

    /// Finds what each viewer of a tile gains and loses after its subjects changed.
    fn settle(&mut self, membership: &Membership, settled: &mut Settled) {
        let Some(stream) = self.tiles.get(&membership.tile) else {
            return;
        };
        let after = &stream.subjects;
        let shifted: [Vec<EntityId>; LAYERS - 1] = [0, 1]
            .map(|layer| prefix_difference(&membership.before, after, layer_ranks(layer).end));
        for subject in shifted.iter().flatten() {
            let stayed = membership.before.binary_search(subject).is_ok()
                && after.binary_search(subject).is_ok();
            if stayed {
                // The subject changed layer, and a viewer may receive the two layers at two
                // tiers.
                self.codec.refresh(*subject);
            }
        }
        let candidates = &mut self.candidates;
        for viewer in &stream.viewers {
            let Some(state) = self.viewers.get_mut(viewer) else {
                continue;
            };
            let visible = state
                .subscriptions
                .get(&membership.tile)
                .map_or(0, Subscription::visible_ranks);
            let boundary = (0..LAYERS - 1)
                .zip(&shifted)
                .find(|(layer, _)| layer_ranks(*layer).end == visible);
            let candidates: &[EntityId] = match boundary {
                Some((_, shifted)) => {
                    candidates.clear();
                    candidates.extend(&membership.crossed);
                    candidates.extend(shifted);
                    candidates.sort_unstable();
                    candidates.dedup();
                    candidates
                }
                None => &membership.crossed,
            };
            for &subject in candidates {
                let wanted = wanted(&self.tiles, &self.homes, state, subject);
                let held = state.held.contains(&subject);
                match wanted {
                    Some(tier) if !held => {
                        state.held.insert(subject);
                        settled
                            .gained
                            .entry(*viewer)
                            .or_default()
                            .push(Gain { subject, tier });
                    }
                    None if held => {
                        state.held.remove(&subject);
                        settled.lost.entry(*viewer).or_default().push(subject);
                    }
                    _ => {}
                }
            }
        }
    }

    /// Encodes each tile and tier that is due and has events to send, and appends the bytes to
    /// the viewers of that tier.
    fn send_moves(&mut self, settled: &Settled) {
        let mut tiles = mem::take(&mut self.scratch_tiles);
        tiles.clear();
        tiles.extend(self.unsent.iter().copied());
        let mut receivers = mem::take(&mut self.scratch_viewers);
        for &tile in &tiles {
            let Some(stream) = self.tiles.get_mut(&tile) else {
                self.unsent.remove(&tile);
                continue;
            };
            if stream.subjects.is_empty() {
                stream.unsent = [false; Tier::ALL.len()];
            }
            for tier in Tier::ALL {
                let unsent = stream.unsent.get(tier.index()).copied().unwrap_or(false);
                if !unsent || !due(tile, tier, self.tick) {
                    continue;
                }
                receivers.clear();
                receivers.extend(stream.viewers.iter().filter(|viewer| {
                    self.viewers
                        .get(viewer)
                        .and_then(|state| state.subscriptions.get(&tile))
                        .is_some_and(|subscription| {
                            subscription
                                .layers
                                .iter()
                                .any(|layer| layer.tier == Some(tier))
                        })
                }));
                if receivers.is_empty() {
                    continue;
                }
                let blob = self
                    .codec
                    .blob(self.tick, tier, &stream.subjects, BlobKind::Delta);
                if let Some(unsent) = stream.unsent.get_mut(tier.index()) {
                    *unsent = false;
                }
                for &viewer in &receivers {
                    let Some(state) = self.viewers.get_mut(&viewer) else {
                        continue;
                    };
                    let gained = settled.gained.get(&viewer).map_or(&[][..], Vec::as_slice);
                    state.append_tier(tile, stream, tier, &blob, gained);
                }
            }
            if stream.unsent.iter().all(|unsent| !unsent) {
                self.unsent.remove(&tile);
            }
        }
        self.scratch_tiles = tiles;
        self.scratch_viewers = receivers;
    }

    fn send_settled(&mut self, settled: Settled) {
        for (viewer, gains) in settled.gained {
            let Some(state) = self.viewers.get_mut(&viewer) else {
                continue;
            };
            for Gain { subject, tier } in gains {
                self.codec
                    .hold(viewer, Hold::Gain { subject, tier }, &mut state.chunks);
            }
        }
        for (viewer, subjects) in settled.lost {
            self.send_removal(viewer, &subjects);
        }
    }

    fn send_removal(&mut self, viewer: ViewerId, subjects: &[EntityId]) {
        if subjects.is_empty() {
            return;
        }
        let mut chunks = Vec::new();
        self.codec
            .hold(viewer, Hold::Lose { subjects }, &mut chunks);
        if let Some(state) = self.viewers.get_mut(&viewer) {
            state.chunks.append(&mut chunks);
        }
    }

    /// Whether a plan of `state` would change nothing: its last plan changed nothing, and since
    /// then it kept its position, no tile of its view changed its subjects, and the budget does
    /// not act.
    fn is_settled(&self, state: &Viewer, memberships: &[Membership]) -> bool {
        state.settled_at == Some(self.position(&state.view))
            && !planner::reads_budget(state.measured)
            && !memberships
                .iter()
                .any(|membership| state.view.contains(membership.tile))
    }

    /// Where the planner measures the distances of the tiles of `view` from.
    fn position(&self, view: &View) -> Point {
        let width = f64::from(self.tile_width.get());
        match view
            .subject
            .and_then(|subject| self.codec.position(subject))
        {
            Some([x, _, z]) => Point { x, z },
            None => Point {
                x: (f64::from(view.center.x) + 0.5) * width,
                z: (f64::from(view.center.z) + 0.5) * width,
            },
        }
    }

    /// Plans the subscriptions of `viewer` and sends what they change: spawns for the layers it
    /// gains, removals for the layers it loses, and keyframes for the layers that change tier.
    fn replan(&mut self, viewer: ViewerId) {
        let Some(state) = self.viewers.get(&viewer) else {
            return;
        };
        let position = self.position(&state.view);
        let mut headers = mem::take(&mut self.headers);
        headers.clear();
        headers.extend(state.view.tiles().map(|tile| {
            TileHeader {
                tile,
                count: self
                    .tiles
                    .get(&tile)
                    .map_or(0, |stream| stream.subjects.len()),
            }
        }));
        let changes = planner::plan(
            self.tile_width,
            position,
            &state.subscriptions,
            &headers,
            state.measured,
        );
        self.headers = headers;
        if let Some(state) = self.viewers.get_mut(&viewer) {
            state.settled_at = changes.is_empty().then_some(position);
        }
        let mut lost = Vec::new();
        for change in changes {
            self.resubscribe(viewer, change, &mut lost);
        }
        self.send_removal(viewer, &lost);
    }

    fn resubscribe(
        &mut self,
        viewer: ViewerId,
        change: SubscriptionChange,
        lost: &mut Vec<EntityId>,
    ) {
        let Some(state) = self.viewers.get_mut(&viewer) else {
            return;
        };
        let tile = change.tile;
        let old = state.subscriptions.get(&tile).copied().unwrap_or_default();
        let new = change.subscription.unwrap_or_default();
        let stream = self.tiles.entry(tile).or_default();
        let own = state.view.subject;
        for (layer, (was, now)) in old.layers.iter().zip(&new.layers).enumerate() {
            let members = stream.layer(layer);
            match (was.tier, now.tier) {
                (None, Some(tier)) => {
                    for &subject in members {
                        if Some(subject) != own && state.held.insert(subject) {
                            self.codec.hold(
                                viewer,
                                Hold::Gain { subject, tier },
                                &mut state.chunks,
                            );
                        }
                    }
                }
                (Some(_), None) => {
                    lost.extend(members.iter().filter(|subject| state.held.remove(subject)));
                }
                (Some(from), Some(to)) if from != to => {
                    let Some(window) = stream.window(layer) else {
                        continue;
                    };
                    let blob = self
                        .keyframes
                        .entry(TileTier { tile, tier: to })
                        .or_insert_with(|| {
                            self.codec
                                .blob(self.tick, to, &stream.subjects, BlobKind::Keyframe)
                        });
                    blob.append_kept(&mut state.chunks, |subject| {
                        window.contains(&subject) && Some(subject) != own
                    });
                }
                _ => {}
            }
        }
        match change.subscription {
            Some(subscription) => {
                stream.viewers.insert(viewer);
                state.subscriptions.insert(tile, subscription);
            }
            None => {
                stream.viewers.remove(&viewer);
                state.subscriptions.remove(&tile);
                self.forget_if_empty(tile);
            }
        }
    }

    fn forget_if_empty(&mut self, tile: TileKey) {
        if self.tiles.get(&tile).is_some_and(TileStream::is_empty) {
            self.tiles.remove(&tile);
        }
    }
}

/// The tier at which the client of `state` holds `subject`, or `None` when it does not hold it.
fn wanted(
    tiles: &BTreeMap<TileKey, TileStream>,
    homes: &BTreeMap<EntityId, TileKey>,
    state: &Viewer,
    subject: EntityId,
) -> Option<Tier> {
    if Some(subject) == state.view.subject {
        return None;
    }
    let tile = homes.get(&subject)?;
    let rank = tiles.get(tile)?.rank(subject)?;
    state.subscriptions.get(tile)?.tier_of(rank)
}

/// Where a ranged subject is for a viewer: the reach of its tile and its range.
#[derive(Clone, Copy, Debug)]
struct Place {
    reach: TileReach,
    range: u32,
}

/// Whether the client of `state` holds the ranged `subject` at `place`, after this tick. A subject
/// with no place, outside the region of the viewer or gone, is not held.
fn wants<C: ReplicationCodec>(
    codec: &C,
    state: &Viewer,
    subject: EntityId,
    place: Option<Place>,
) -> bool {
    let Some(place) = place else {
        return false;
    };
    if Some(subject) == state.view.subject {
        return false;
    }
    let held = state.held.contains(&subject);
    tile_reach::tracks(place.reach, place.range, held, || {
        let viewer = state.tracked_at?;
        let [x, _, z] = codec.position(subject)?;
        Some((x - viewer.x).hypot(z - viewer.z))
    })
}

/// Records that the client of `viewer` holds `subject` or not, and what it gains or loses.
fn record_hold(
    state: &mut Viewer,
    viewer: ViewerId,
    subject: EntityId,
    wanted: bool,
    settled: &mut Settled,
) {
    if wanted {
        if state.held.insert(subject) {
            settled.gained.entry(viewer).or_default().push(Gain {
                subject,
                tier: Tier::NEAREST,
            });
        }
    } else if state.held.remove(&subject) {
        settled.lost.entry(viewer).or_default().push(subject);
    }
}

/// Whether tier `tier` of `tile` sends its moves at `tick`. The phase of a tile spreads the work
/// of a slow tier over its ticks.
fn due(tile: TileKey, tier: Tier, tick: u64) -> bool {
    let phase = (u64::from(tile.x as u32).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ u64::from(tile.y as u32).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ u64::from(tile.z as u32).wrapping_mul(0x1656_67B1_9E37_79F9))
        >> 32;
    tick.wrapping_add(phase) % tier.interval() == 0
}

/// The subjects in exactly one of the first `end` of `before` and the first `end` of `after`.
fn prefix_difference(before: &[EntityId], after: &[EntityId], end: usize) -> Vec<EntityId> {
    let old = before.get(..end).unwrap_or(before);
    let new = after.get(..end).unwrap_or(after);
    let gone = old
        .iter()
        .filter(|subject| new.binary_search(subject).is_err());
    let come = new
        .iter()
        .filter(|subject| old.binary_search(subject).is_err());
    gone.chain(come).copied().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;

    use crate::tile_blob::SubjectRange;
    use crate::tile_radius::TileRadius;

    /// Writes each event as a short token, so the bytes of a viewer read as text.
    #[derive(Default)]
    struct TextCodec {
        encodes: usize,
        /// The subjects that moved since the last encode of each tier.
        moved: [BTreeSet<EntityId>; 4],
        /// Bytes for one viewer at the end of the next tick.
        owed: Vec<ViewerBytes>,
        ranges: BTreeMap<EntityId, u32>,
        positions: BTreeMap<EntityId, [f64; 3]>,
    }

    impl TextCodec {
        fn blob(subjects: impl IntoIterator<Item = EntityId>, token: &str) -> TileBlob {
            let mut text = String::new();
            let mut ranges = Vec::new();
            for subject in subjects {
                let start = text.len();
                text.push_str(&format!("{token} {};", subject.0));
                ranges.push(SubjectRange {
                    subject,
                    range: start..text.len(),
                });
            }
            TileBlob {
                bytes: Bytes::from(text),
                ranges,
            }
        }
    }

    impl ReplicationCodec for TextCodec {
        type Data = ();
        type Input = ();

        fn apply(&mut self, _tick: u64, delta: &TileDelta<()>) {
            for SubjectEvent { subject, event } in &delta.events {
                if matches!(event, EntityEvent::Move(())) {
                    for moved in &mut self.moved {
                        moved.insert(*subject);
                    }
                }
            }
        }

        fn blob(
            &mut self,
            _tick: u64,
            tier: Tier,
            subjects: &[EntityId],
            kind: BlobKind,
        ) -> TileBlob {
            match kind {
                BlobKind::Keyframe => Self::blob(subjects.iter().copied(), "key"),
                BlobKind::Delta => {
                    self.encodes += 1;
                    let moved = &mut self.moved[tier.index()];
                    let sent: Vec<EntityId> = subjects
                        .iter()
                        .copied()
                        .filter(|subject| moved.remove(subject))
                        .collect();
                    Self::blob(sent, "move")
                }
            }
        }

        fn refresh(&mut self, _subject: EntityId) {}

        fn hold(&mut self, viewer: ViewerId, change: Hold<'_>, chunks: &mut Vec<Bytes>) {
            match change {
                Hold::Gain { subject, .. } => {
                    chunks.push(Bytes::from(format!("bundle {};", subject.0)));
                }
                Hold::Lose { subjects } => {
                    let ids: Vec<String> = subjects.iter().map(|id| id.0.to_string()).collect();
                    chunks.push(Bytes::from(format!("remove {};", ids.join(","))));
                }
                Hold::Forget => self.owed.retain(|owed| owed.viewer != viewer),
            }
        }

        fn finish_tick(&mut self, _tick: u64, out: &mut Vec<ViewerBytes>) {
            out.append(&mut self.owed);
        }

        fn receive(&mut self, (): ()) {}

        fn tracking_range(&self, subject: EntityId) -> Option<u32> {
            self.ranges.get(&subject).copied()
        }

        fn position(&self, subject: EntityId) -> Option<[f64; 3]> {
            self.positions.get(&subject).copied()
        }
    }

    const ORIGIN: TileKey = TileKey::new(0, 0, 0);
    const FAR: TileKey = TileKey::new(10, 0, 0);

    fn muxer() -> Muxer<TextCodec> {
        Muxer::new(TextCodec::default(), NonZeroU32::new(32).unwrap())
    }

    fn view(center: TileKey, subject: Option<u64>) -> View {
        View {
            center,
            radius: TileRadius::new(1, 0, 1),
            subject: subject.map(EntityId),
        }
    }

    fn event(subject: u64, event: EntityEvent<()>) -> SubjectEvent<()> {
        SubjectEvent {
            subject: EntityId(subject),
            event,
        }
    }

    /// Publishes `tiles` at `tick`, then an empty tick, so tier 0 is due in one of the two.
    fn publish(muxer: &mut Muxer<TextCodec>, tick: u64, tiles: Vec<TileDelta<()>>) {
        muxer.publish(&TileDeltas { tick, tiles });
        muxer.publish(&TileDeltas {
            tick: tick + 1,
            tiles: Vec::new(),
        });
    }

    fn at(tile: TileKey, events: Vec<SubjectEvent<()>>) -> Vec<TileDelta<()>> {
        vec![TileDelta { tile, events }]
    }

    fn text(muxer: &mut Muxer<TextCodec>) -> BTreeMap<u64, String> {
        muxer
            .take_output()
            .map(|output| {
                let bytes: Vec<u8> = output.chunks.concat();
                (output.viewer.0, String::from_utf8(bytes).unwrap())
            })
            .collect()
    }

    #[test]
    fn a_new_viewer_gets_the_spawns_of_the_tile_then_its_moves_but_never_its_own_subject() {
        let mut muxer = muxer();
        let spawns = vec![
            event(1, EntityEvent::Spawn(())),
            event(2, EntityEvent::Spawn(())),
        ];
        publish(&mut muxer, 0, at(ORIGIN, spawns));

        muxer.subscribe(ViewerId(7), view(ORIGIN, Some(1)));
        assert_eq!(text(&mut muxer), BTreeMap::from([(7, "bundle 2;".into())]));

        let moves = vec![
            event(1, EntityEvent::Move(())),
            event(2, EntityEvent::Move(())),
        ];
        publish(&mut muxer, 2, at(ORIGIN, moves));
        assert_eq!(text(&mut muxer), BTreeMap::from([(7, "move 2;".into())]));
    }

    #[test]
    fn a_subject_that_crosses_tiles_is_removed_where_it_leaves_view_and_spawned_where_it_enters() {
        let mut muxer = muxer();
        muxer.subscribe(ViewerId(1), view(ORIGIN, None));
        muxer.subscribe(ViewerId(2), view(FAR, None));
        publish(
            &mut muxer,
            0,
            at(ORIGIN, vec![event(5, EntityEvent::Spawn(()))]),
        );
        text(&mut muxer);

        let crossing = vec![
            TileDelta {
                tile: ORIGIN,
                events: vec![event(5, EntityEvent::Leave { to: FAR })],
            },
            TileDelta {
                tile: FAR,
                events: vec![
                    event(5, EntityEvent::Enter { from: ORIGIN }),
                    event(5, EntityEvent::Move(())),
                ],
            },
        ];
        publish(&mut muxer, 2, crossing);

        assert_eq!(
            text(&mut muxer),
            BTreeMap::from([(1, "remove 5;".into()), (2, "bundle 5;".into())])
        );
    }

    #[test]
    fn a_viewer_that_moves_away_gets_the_removal_of_the_subjects_it_no_longer_sees() {
        let mut muxer = muxer();
        publish(
            &mut muxer,
            0,
            at(ORIGIN, vec![event(3, EntityEvent::Spawn(()))]),
        );
        muxer.subscribe(ViewerId(1), view(ORIGIN, None));
        text(&mut muxer);

        muxer.subscribe(ViewerId(1), view(FAR, None));
        assert_eq!(text(&mut muxer), BTreeMap::from([(1, "remove 3;".into())]));
    }

    #[test]
    fn the_bytes_of_a_viewer_that_leaves_still_come_out_and_none_after_them() {
        let mut muxer = muxer();
        publish(
            &mut muxer,
            0,
            at(ORIGIN, vec![event(3, EntityEvent::Spawn(()))]),
        );
        muxer.subscribe(ViewerId(4), view(ORIGIN, None));
        muxer.codec_mut().owed.push(ViewerBytes {
            viewer: ViewerId(4),
            chunks: vec![Bytes::from_static(b"later;")],
        });

        muxer.unsubscribe(ViewerId(4));
        publish(&mut muxer, 2, Vec::new());

        assert_eq!(text(&mut muxer), BTreeMap::from([(4, "bundle 3;".into())]));
    }

    #[test]
    fn each_tile_is_encoded_once_for_its_tier_and_its_bytes_are_shared_by_all_viewers() {
        let mut muxer = muxer();
        publish(
            &mut muxer,
            0,
            at(ORIGIN, vec![event(9, EntityEvent::Spawn(()))]),
        );
        for viewer in 1..=3 {
            muxer.subscribe(ViewerId(viewer), view(ORIGIN, None));
        }
        text(&mut muxer);
        let before = muxer.codec().encodes;

        publish(
            &mut muxer,
            2,
            at(ORIGIN, vec![event(9, EntityEvent::Move(()))]),
        );

        assert_eq!(muxer.codec().encodes - before, 1);
        let outputs: Vec<ViewerBytes> = muxer.take_output().collect();
        assert_eq!(outputs.len(), 3);
        let first = outputs[0].chunks[0].as_ptr();
        assert!(
            outputs
                .iter()
                .all(|output| output.chunks[0].as_ptr() == first),
            "every viewer holds a view into the one encoded buffer"
        );
    }

    #[test]
    fn a_crowd_past_the_cap_is_cut_by_layer_and_the_viewer_holds_no_more_than_k() {
        let mut muxer = muxer();
        let spawns: Vec<SubjectEvent<()>> = (1..=300)
            .map(|subject| event(subject, EntityEvent::Spawn(())))
            .collect();
        publish(&mut muxer, 0, at(ORIGIN, spawns));
        muxer.subscribe(ViewerId(1), view(ORIGIN, None));
        let spawned = text(&mut muxer)[&1].matches("bundle").count();
        assert_eq!(spawned, 96, "layers 0 and 1 fit in 200, layer 2 does not");

        let moves: Vec<SubjectEvent<()>> = (1..=300)
            .map(|subject| event(subject, EntityEvent::Move(())))
            .collect();
        // Four ticks, so the tiers of layers 0 and 1 are each due once.
        publish(&mut muxer, 2, at(ORIGIN, moves));
        publish(&mut muxer, 4, Vec::new());
        let moved = text(&mut muxer)[&1].matches("move").count();
        assert_eq!(
            moved, 96,
            "the viewer gets the moves of layers 0 and 1 only"
        );
    }

    const RANGE: u32 = 160;

    /// Places a ranged subject with `RANGE` at `x`, `z`, and gives the delta of its spawn.
    fn spawn_ranged(muxer: &mut Muxer<TextCodec>, subject: u64, x: f64, z: f64) -> TileDelta<()> {
        let codec = muxer.codec_mut();
        codec.ranges.insert(EntityId(subject), RANGE);
        codec.positions.insert(EntityId(subject), [x, 0.0, z]);
        TileDelta {
            tile: TileKey::new((x / 32.0).floor() as i32, 0, (z / 32.0).floor() as i32),
            events: vec![event(subject, EntityEvent::Spawn(()))],
        }
    }

    fn held_by(text: &str, token: &str) -> BTreeSet<u64> {
        text.split(';')
            .filter_map(|part| part.strip_prefix(token))
            .flat_map(|ids| ids.split(','))
            .map(|id| id.trim().parse().unwrap())
            .collect()
    }

    #[test]
    fn a_viewer_holds_each_ranged_subject_within_its_range_and_still_at_most_k_others() {
        let mut muxer = muxer();
        muxer
            .codec_mut()
            .positions
            .insert(EntityId(1), [0.5, 0.0, 0.5]);
        let players: Vec<SubjectEvent<()>> = (1..=300)
            .map(|subject| event(subject, EntityEvent::Spawn(())))
            .collect();
        let mut tiles = at(ORIGIN, players);
        let mut expected = BTreeSet::new();
        let mut near = BTreeSet::new();
        let mut subject = 1000;
        for x in (-200..=200).step_by(8) {
            for z in (-200..=200).step_by(8) {
                let (x, z) = (f64::from(x) + 0.25, f64::from(z) + 0.75);
                tiles.push(spawn_ranged(&mut muxer, subject, x, z));
                let distance = (x - 0.5).hypot(z - 0.5);
                if distance <= f64::from(RANGE) {
                    expected.insert(subject);
                }
                if distance <= 16.0 {
                    near.insert(subject);
                }
                subject += 1;
            }
        }
        publish(&mut muxer, 0, tiles);

        muxer.subscribe(ViewerId(1), view(ORIGIN, Some(1)));
        let bundles = held_by(&text(&mut muxer)[&1], "bundle ");

        let ranged: BTreeSet<u64> = bundles.iter().copied().filter(|id| *id >= 1000).collect();
        assert!(
            expected.len() > 1000,
            "the scene puts more than 1,000 subjects in range"
        );
        assert!(
            near.len() > 10 && near.is_subset(&ranged),
            "the nearest subjects are held"
        );
        assert_eq!(
            ranged, expected,
            "exactly the ranged subjects within the range"
        );
        assert_eq!(
            bundles.len() - ranged.len(),
            95,
            "layers 0 and 1 of the players fit in K, without the own player"
        );
    }

    #[test]
    fn a_ranged_subject_is_removed_when_it_walks_out_of_range_and_spawned_when_it_comes_back() {
        let mut muxer = muxer();
        let home = TileKey::new(-1, 0, 0);
        muxer
            .codec_mut()
            .positions
            .insert(EntityId(1), [-5.0, 0.0, 0.5]);
        let mut tiles = at(home, vec![event(1, EntityEvent::Spawn(()))]);
        tiles.push(spawn_ranged(&mut muxer, 2000, 150.0, 0.5));
        publish(&mut muxer, 0, tiles);
        muxer.subscribe(ViewerId(1), view(home, Some(1)));
        assert_eq!(text(&mut muxer)[&1], "bundle 2000;");

        let edge = TileKey::new(4, 0, 0);
        let walk = |muxer: &mut Muxer<TextCodec>, tick: u64, x: f64, tiles: Vec<TileDelta<()>>| {
            muxer
                .codec_mut()
                .positions
                .insert(EntityId(2000), [x, 0.0, 0.5]);
            publish(muxer, tick, tiles);
            text(muxer).remove(&1).unwrap_or_default()
        };
        let moved = || at(edge, vec![event(2000, EntityEvent::Move(()))]);
        assert_eq!(walk(&mut muxer, 2, 154.9, moved()), "");
        assert_eq!(walk(&mut muxer, 4, 156.0, moved()), "", "inside the margin");
        assert_eq!(walk(&mut muxer, 6, 156.6, moved()), "remove 2000;");
        assert_eq!(walk(&mut muxer, 8, 154.0, moved()), "bundle 2000;");

        let far = TileKey::new(6, 0, 0);
        let crossing = vec![
            TileDelta {
                tile: edge,
                events: vec![event(2000, EntityEvent::Leave { to: far })],
            },
            TileDelta {
                tile: far,
                events: vec![event(2000, EntityEvent::Enter { from: edge })],
            },
        ];
        assert_eq!(walk(&mut muxer, 10, 200.0, crossing), "remove 2000;");
    }
}

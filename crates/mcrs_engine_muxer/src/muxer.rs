use std::collections::{BTreeMap, BTreeSet};
use std::mem;

use crate::entity_event::{EntityEvent, SubjectEvent};
use crate::entity_id::{EntityId, ViewerId};
use crate::muxer_settings::MuxerSettings;
use crate::planner::{Point, SubscriptionChange, TICKS_PER_SECOND, TileHeader, ViewerState, plan};
use crate::replication_codec::{BlobKind, Hold, ReplicationCodec};
use crate::subscription::{LAYERS, Subscription, layer_ranks};
use crate::tier::Tier;
use crate::tile_blob::TileBlob;
use crate::tile_delta::{TileDelta, TileDeltas};
use crate::tile_key::TileKey;
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
#[derive(Debug)]
pub struct Muxer<C> {
    codec: C,
    settings: MuxerSettings,
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
    tick: u64,
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
    pub fn new(codec: C, settings: MuxerSettings) -> Self {
        Self {
            codec,
            settings,
            tiles: BTreeMap::new(),
            homes: BTreeMap::new(),
            viewers: BTreeMap::new(),
            loose: Vec::new(),
            owed: Vec::new(),
            unsent: BTreeSet::new(),
            keyframes: BTreeMap::new(),
            tick: 0,
        }
    }

    #[must_use]
    pub fn codec(&self) -> &C {
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

        let budget = u64::from(self.settings.budget) / TICKS_PER_SECOND;
        for state in self.viewers.values_mut() {
            state.measure(budget);
        }
        let viewers: Vec<ViewerId> = self.viewers.keys().copied().collect();
        for viewer in viewers {
            self.replan(viewer);
        }
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

    /// Moves the subjects that come into or go out of the tile of `delta`.
    fn take_in(&mut self, delta: &TileDelta<C::Data>) -> Option<Membership> {
        let tile = delta.tile;
        let stream = self.tiles.entry(tile).or_default();
        if !delta.events.is_empty() {
            stream.unsent = [true; Tier::ALL.len()];
            self.unsent.insert(tile);
        }
        let mut before: Option<Vec<EntityId>> = None;
        let mut crossed = Vec::new();
        for SubjectEvent { subject, event } in &delta.events {
            let subject = *subject;
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
        Some(Membership {
            tile,
            before: before?,
            crossed,
        })
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
        for viewer in &stream.viewers {
            let Some(state) = self.viewers.get_mut(viewer) else {
                continue;
            };
            let visible = state
                .subscriptions
                .get(&membership.tile)
                .map_or(0, Subscription::visible_ranks);
            let mut candidates = membership.crossed.clone();
            let boundary = (0..LAYERS - 1)
                .zip(&shifted)
                .find(|(layer, _)| layer_ranks(*layer).end == visible);
            if let Some((_, shifted)) = boundary {
                candidates.extend(shifted);
            }
            candidates.sort_unstable();
            candidates.dedup();
            for subject in candidates {
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
        let tiles: Vec<TileKey> = self.unsent.iter().copied().collect();
        for tile in tiles {
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
                let receivers: Vec<ViewerId> = stream
                    .viewers
                    .iter()
                    .filter(|viewer| {
                        self.viewers
                            .get(viewer)
                            .and_then(|state| state.subscriptions.get(&tile))
                            .is_some_and(|subscription| {
                                subscription
                                    .layers
                                    .iter()
                                    .any(|layer| layer.tier == Some(tier))
                            })
                    })
                    .copied()
                    .collect();
                if receivers.is_empty() {
                    continue;
                }
                let blob = self
                    .codec
                    .blob(self.tick, tier, &stream.subjects, BlobKind::Delta);
                if let Some(unsent) = stream.unsent.get_mut(tier.index()) {
                    *unsent = false;
                }
                for viewer in receivers {
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

    /// Plans the subscriptions of `viewer` and sends what they change: spawns for the layers it
    /// gains, removals for the layers it loses, and keyframes for the layers that change tier.
    fn replan(&mut self, viewer: ViewerId) {
        let Some(state) = self.viewers.get(&viewer) else {
            return;
        };
        let view = state.view;
        let width = f64::from(self.settings.tile_width.get());
        let position = match view
            .subject
            .and_then(|subject| self.codec.position(subject))
        {
            Some([x, _, z]) => Point { x, z },
            None => Point {
                x: (f64::from(view.center.x) + 0.5) * width,
                z: (f64::from(view.center.z) + 0.5) * width,
            },
        };
        let headers: Vec<TileHeader> = view
            .tiles()
            .map(|tile| TileHeader {
                tile,
                count: self
                    .tiles
                    .get(&tile)
                    .map_or(0, |stream| stream.subjects.len()),
            })
            .collect();
        let viewer_state = ViewerState {
            settings: &self.settings,
            position,
            subscriptions: &state.subscriptions,
        };
        let changes = plan(&viewer_state, &headers, state.measured);
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
                        window.contains(subject) && Some(subject) != own
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

        fn position(&self, _subject: EntityId) -> Option<[f64; 3]> {
            None
        }
    }

    const ORIGIN: TileKey = TileKey::new(0, 0, 0);
    const FAR: TileKey = TileKey::new(10, 0, 0);

    fn muxer() -> Muxer<TextCodec> {
        Muxer::new(TextCodec::default(), MuxerSettings::default())
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
        let mut muxer = Muxer::new(
            TextCodec::default(),
            MuxerSettings {
                cap: 50,
                ..MuxerSettings::default()
            },
        );
        let spawns: Vec<SubjectEvent<()>> = (1..=120)
            .map(|subject| event(subject, EntityEvent::Spawn(())))
            .collect();
        publish(&mut muxer, 0, at(ORIGIN, spawns));
        muxer.subscribe(ViewerId(1), view(ORIGIN, None));
        let spawned = text(&mut muxer)[&1].matches("bundle").count();
        assert_eq!(spawned, 32, "layer 0 fits in 50, layer 1 does not");

        let moves: Vec<SubjectEvent<()>> = (1..=120)
            .map(|subject| event(subject, EntityEvent::Move(())))
            .collect();
        publish(&mut muxer, 2, at(ORIGIN, moves));
        let moved = text(&mut muxer)[&1].matches("move").count();
        assert_eq!(moved, 32, "the viewer gets the moves of layer 0 only");
    }
}

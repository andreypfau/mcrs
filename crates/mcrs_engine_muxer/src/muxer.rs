use std::collections::{BTreeMap, BTreeSet};
use std::mem;
use std::ops::Range;

use bytes::Bytes;

use crate::entity_event::{EntityEvent, SubjectEvent};
use crate::entity_id::{EntityId, ViewerId};
use crate::replication_codec::ReplicationCodec;
use crate::tile_blob::{SubjectRange, TileBlob};
use crate::tile_delta::{TileDelta, TileDeltas};
use crate::tile_key::TileKey;
use crate::tile_stream::TileStream;
use crate::view::View;
use crate::viewer_bytes::ViewerBytes;

/// Decides which entity bytes each viewer receives. It starts no thread and reads no clock, so
/// a game can call it from its own loop.
///
/// The work of a call is proportional to the changes and the subscriptions it touches, not to
/// subjects times viewers. Each tile delta is encoded once, and its viewers share the bytes.
#[derive(Debug)]
pub struct Muxer<C> {
    codec: C,
    tiles: BTreeMap<TileKey, TileStream>,
    viewers: BTreeMap<ViewerId, Viewer>,
    /// The viewers whose chunks are not taken yet.
    pending: BTreeSet<ViewerId>,
    /// The bytes of viewers that no longer subscribe.
    loose: Vec<ViewerBytes>,
    owed: Vec<ViewerBytes>,
}

#[derive(Debug)]
struct Viewer {
    view: View,
    chunks: Vec<Bytes>,
}

/// A subject that comes into a tile or goes out of it. `other` is the tile on the other side,
/// or `None` when the subject spawns or despawns.
struct Crossing {
    subject: EntityId,
    other: Option<TileKey>,
}

impl Crossing {
    /// Whether a viewer of `view` gains or loses the subject on this crossing.
    fn changes(&self, view: &View) -> bool {
        Some(self.subject) != view.subject && self.other.is_none_or(|other| !view.contains(other))
    }
}

impl<C: ReplicationCodec> Muxer<C> {
    #[must_use]
    pub fn new(codec: C) -> Self {
        Self {
            codec,
            tiles: BTreeMap::new(),
            viewers: BTreeMap::new(),
            pending: BTreeSet::new(),
            loose: Vec::new(),
            owed: Vec::new(),
        }
    }

    #[must_use]
    pub fn codec(&self) -> &C {
        &self.codec
    }

    pub fn codec_mut(&mut self) -> &mut C {
        &mut self.codec
    }

    /// Takes in the changes of one tick of a world.
    pub fn publish(&mut self, deltas: &TileDeltas<C::Data>) {
        let mut removals: BTreeMap<ViewerId, Vec<EntityId>> = BTreeMap::new();
        for delta in &deltas.tiles {
            self.publish_tile(deltas.tick, delta, &mut removals);
        }
        for (viewer, subjects) in removals {
            let removal = self.codec.remove(viewer, &subjects);
            self.push(viewer, removal);
        }
        let mut owed = mem::take(&mut self.owed);
        self.codec.finish_tick(deltas.tick, &mut owed);
        for ViewerBytes { viewer, mut chunks } in owed.drain(..) {
            match self.viewers.get_mut(&viewer) {
                Some(state) => {
                    state.chunks.append(&mut chunks);
                    self.pending.insert(viewer);
                }
                None => self.loose.push(ViewerBytes { viewer, chunks }),
            }
        }
        self.owed = owed;
    }

    /// Subscribes `viewer` to the tiles of `view`. For a viewer that is already subscribed, only
    /// the difference to its last view costs work: it gets the spawn bundles of the subjects in
    /// the tiles it gains, and the removal of the subjects in the tiles it loses.
    pub fn subscribe(&mut self, viewer: ViewerId, view: View) {
        let previous: Option<View> = self.viewers.get(&viewer).map(|state| state.view);
        // A new own subject changes what the viewer may hold in every tile, so the whole view
        // starts again.
        let kept: Option<View> = previous.filter(|old| old.subject == view.subject);
        let mut chunks = Vec::new();

        if let Some(old) = previous {
            let mut removed = Vec::new();
            for tile in old.tiles() {
                if kept.is_some_and(|_| view.contains(tile)) {
                    continue;
                }
                self.leave_tile(viewer, tile, old.subject, &mut removed);
            }
            if !removed.is_empty() {
                chunks.push(self.codec.remove(viewer, &removed));
            }
        }

        for tile in view.tiles() {
            if kept.is_some_and(|old| old.contains(tile)) {
                continue;
            }
            let stream = self.tiles.entry(tile).or_default();
            stream.viewers.insert(viewer);
            for &subject in &stream.subjects {
                if Some(subject) != view.subject {
                    self.codec.spawn(viewer, subject, &mut chunks);
                }
            }
        }

        let state = self.viewers.entry(viewer).or_insert_with(|| Viewer {
            view,
            chunks: Vec::new(),
        });
        state.view = view;
        if !chunks.is_empty() {
            state.chunks.append(&mut chunks);
            self.pending.insert(viewer);
        }
    }

    /// Forgets `viewer`. It gets no removal, because its connection is gone or its client drops
    /// its entities by itself. The bytes it has not taken stay for [`Self::take_output`].
    pub fn unsubscribe(&mut self, viewer: ViewerId) {
        let Some(state) = self.viewers.remove(&viewer) else {
            return;
        };
        for tile in state.view.tiles() {
            if let Some(stream) = self.tiles.get_mut(&tile) {
                stream.viewers.remove(&viewer);
                if stream.is_empty() {
                    self.tiles.remove(&tile);
                }
            }
        }
        if self.pending.remove(&viewer) && !state.chunks.is_empty() {
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
        let Self {
            viewers,
            pending,
            loose,
            ..
        } = self;
        loose
            .drain(..)
            .chain(mem::take(pending).into_iter().filter_map(|viewer| {
                let state = viewers.get_mut(&viewer)?;
                Some(ViewerBytes {
                    viewer,
                    chunks: mem::take(&mut state.chunks),
                })
            }))
    }

    fn push(&mut self, viewer: ViewerId, chunk: Bytes) {
        if let Some(state) = self.viewers.get_mut(&viewer) {
            state.chunks.push(chunk);
            self.pending.insert(viewer);
        }
    }

    fn publish_tile(
        &mut self,
        tick: u64,
        delta: &TileDelta<C::Data>,
        removals: &mut BTreeMap<ViewerId, Vec<EntityId>>,
    ) {
        let stream = self.tiles.entry(delta.tile).or_default();
        let mut entered = Vec::new();
        let mut left = Vec::new();
        for SubjectEvent { subject, event } in &delta.events {
            let subject = *subject;
            let (entering, other) = match event {
                EntityEvent::Spawn(_) => (true, None),
                EntityEvent::Enter { from } => (true, Some(*from)),
                EntityEvent::Leave { to } => (false, Some(*to)),
                EntityEvent::Despawn => (false, None),
                EntityEvent::Move(_) | EntityEvent::Data(_) => continue,
            };
            let crossing = Crossing { subject, other };
            if entering {
                stream.subjects.insert(subject);
                entered.push(crossing);
            } else {
                stream.subjects.remove(&subject);
                left.push(crossing);
            }
        }

        let blob = self.codec.encode(tick, delta);
        let mut gained: Vec<EntityId> = Vec::new();
        for &viewer in &stream.viewers {
            let Some(state) = self.viewers.get_mut(&viewer) else {
                continue;
            };
            let own = state.view.subject;
            gained.clear();
            gained.extend(
                entered
                    .iter()
                    .filter(|crossing| crossing.changes(&state.view))
                    .map(|crossing| crossing.subject),
            );
            let before = state.chunks.len();
            append_blob(&mut state.chunks, &blob, |subject| {
                Some(subject) == own || gained.contains(&subject)
            });
            for &subject in &gained {
                self.codec.spawn(viewer, subject, &mut state.chunks);
            }
            for crossing in &left {
                if crossing.changes(&state.view) {
                    removals.entry(viewer).or_default().push(crossing.subject);
                }
            }
            if state.chunks.len() > before {
                self.pending.insert(viewer);
            }
        }

        if stream.is_empty() {
            self.tiles.remove(&delta.tile);
        }
    }

    fn leave_tile(
        &mut self,
        viewer: ViewerId,
        tile: TileKey,
        own: Option<EntityId>,
        removed: &mut Vec<EntityId>,
    ) {
        let Some(stream) = self.tiles.get_mut(&tile) else {
            return;
        };
        stream.viewers.remove(&viewer);
        removed.extend(
            stream
                .subjects
                .iter()
                .copied()
                .filter(|&subject| Some(subject) != own),
        );
        if stream.is_empty() {
            self.tiles.remove(&tile);
        }
    }
}

/// Appends the bytes of `blob` without the ranges of the subjects that `excluded` names.
fn append_blob(chunks: &mut Vec<Bytes>, blob: &TileBlob, excluded: impl Fn(EntityId) -> bool) {
    let len = blob.bytes.len();
    let mut start = 0;
    for SubjectRange { subject, range } in &blob.ranges {
        // A range out of order or out of bounds would make `Bytes::slice` panic, so the muxer
        // keeps those bytes rather than trust the codec.
        let in_bounds = start <= range.start && range.start <= range.end && range.end <= len;
        if !in_bounds || !excluded(*subject) {
            continue;
        }
        append_slice(chunks, &blob.bytes, start..range.start);
        start = range.end;
    }
    append_slice(chunks, &blob.bytes, start..len);
}

fn append_slice(chunks: &mut Vec<Bytes>, bytes: &Bytes, range: Range<usize>) {
    if !range.is_empty() {
        chunks.push(bytes.slice(range));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tile_radius::TileRadius;

    /// Writes each event as a short token, so the bytes of a viewer read as text.
    #[derive(Default)]
    struct TextCodec {
        encodes: usize,
        /// Bytes for one viewer at the end of the next tick.
        owed: Vec<ViewerBytes>,
    }

    impl ReplicationCodec for TextCodec {
        type Data = ();
        type Input = ();

        fn encode(&mut self, _tick: u64, delta: &TileDelta<()>) -> TileBlob {
            self.encodes += 1;
            let mut text = String::new();
            let mut ranges = Vec::new();
            for SubjectEvent { subject, event } in &delta.events {
                let start = text.len();
                let token = match event {
                    EntityEvent::Move(()) => "move",
                    EntityEvent::Data(()) => "data",
                    EntityEvent::Spawn(())
                    | EntityEvent::Despawn
                    | EntityEvent::Enter { .. }
                    | EntityEvent::Leave { .. } => continue,
                };
                text.push_str(&format!("{token} {};", subject.0));
                ranges.push(SubjectRange {
                    subject: *subject,
                    range: start..text.len(),
                });
            }
            TileBlob {
                bytes: Bytes::from(text),
                ranges,
            }
        }

        fn spawn(&mut self, _viewer: ViewerId, subject: EntityId, chunks: &mut Vec<Bytes>) {
            chunks.push(Bytes::from(format!("bundle {};", subject.0)));
        }

        fn remove(&mut self, _viewer: ViewerId, subjects: &[EntityId]) -> Bytes {
            let ids: Vec<String> = subjects.iter().map(|id| id.0.to_string()).collect();
            Bytes::from(format!("remove {};", ids.join(",")))
        }

        fn finish_tick(&mut self, _tick: u64, out: &mut Vec<ViewerBytes>) {
            out.append(&mut self.owed);
        }

        fn receive(&mut self, (): ()) {}
    }

    const ORIGIN: TileKey = TileKey::new(0, 0, 0);
    const FAR: TileKey = TileKey::new(10, 0, 0);

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

    fn tick(tile: TileKey, events: Vec<SubjectEvent<()>>) -> TileDeltas<()> {
        TileDeltas {
            tick: 0,
            tiles: vec![TileDelta { tile, events }],
        }
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
    fn a_new_viewer_gets_the_spawns_of_the_tile_then_its_deltas_but_never_its_own_subject() {
        let mut muxer = Muxer::new(TextCodec::default());
        muxer.publish(&tick(
            ORIGIN,
            vec![
                event(1, EntityEvent::Spawn(())),
                event(2, EntityEvent::Spawn(())),
            ],
        ));

        muxer.subscribe(ViewerId(7), view(ORIGIN, Some(1)));
        assert_eq!(text(&mut muxer), BTreeMap::from([(7, "bundle 2;".into())]));

        muxer.publish(&tick(
            ORIGIN,
            vec![
                event(1, EntityEvent::Move(())),
                event(2, EntityEvent::Move(())),
            ],
        ));
        assert_eq!(text(&mut muxer), BTreeMap::from([(7, "move 2;".into())]));
    }

    #[test]
    fn a_spawn_and_a_despawn_reach_each_viewer_but_the_own_one() {
        let mut muxer = Muxer::new(TextCodec::default());
        muxer.subscribe(ViewerId(1), view(ORIGIN, Some(1)));
        muxer.subscribe(ViewerId(2), view(ORIGIN, Some(2)));

        muxer.publish(&tick(
            ORIGIN,
            vec![
                event(2, EntityEvent::Spawn(())),
                event(2, EntityEvent::Move(())),
            ],
        ));
        assert_eq!(text(&mut muxer), BTreeMap::from([(1, "bundle 2;".into())]));

        muxer.publish(&tick(ORIGIN, vec![event(2, EntityEvent::Despawn)]));
        assert_eq!(text(&mut muxer), BTreeMap::from([(1, "remove 2;".into())]));
    }

    #[test]
    fn a_subject_that_crosses_tiles_is_removed_where_it_leaves_view_and_spawned_where_it_enters() {
        let mut muxer = Muxer::new(TextCodec::default());
        muxer.subscribe(ViewerId(1), view(ORIGIN, None));
        muxer.subscribe(ViewerId(2), view(FAR, None));
        muxer.publish(&tick(ORIGIN, vec![event(5, EntityEvent::Spawn(()))]));
        text(&mut muxer);

        muxer.publish(&TileDeltas {
            tick: 1,
            tiles: vec![
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
            ],
        });

        assert_eq!(
            text(&mut muxer),
            BTreeMap::from([(1, "remove 5;".into()), (2, "bundle 5;".into())])
        );
    }

    #[test]
    fn a_viewer_that_moves_away_gets_the_removal_of_the_subjects_it_no_longer_sees() {
        let mut muxer = Muxer::new(TextCodec::default());
        muxer.publish(&tick(ORIGIN, vec![event(3, EntityEvent::Spawn(()))]));
        muxer.subscribe(ViewerId(1), view(ORIGIN, None));
        text(&mut muxer);

        muxer.subscribe(ViewerId(1), view(FAR, None));
        assert_eq!(text(&mut muxer), BTreeMap::from([(1, "remove 3;".into())]));
    }

    #[test]
    fn bytes_the_codec_owes_a_viewer_that_left_still_come_out() {
        let mut muxer = Muxer::new(TextCodec::default());
        muxer.subscribe(ViewerId(4), view(ORIGIN, None));
        muxer.unsubscribe(ViewerId(4));
        muxer.codec_mut().owed.push(ViewerBytes {
            viewer: ViewerId(4),
            chunks: vec![Bytes::from_static(b"bye;")],
        });

        muxer.publish(&tick(ORIGIN, Vec::new()));

        assert_eq!(text(&mut muxer), BTreeMap::from([(4, "bye;".into())]));
    }

    #[test]
    fn each_tile_delta_is_encoded_once_and_its_bytes_are_shared_by_all_viewers() {
        let mut muxer = Muxer::new(TextCodec::default());
        for viewer in 1..=3 {
            muxer.subscribe(ViewerId(viewer), view(ORIGIN, None));
        }

        muxer.publish(&tick(ORIGIN, vec![event(9, EntityEvent::Move(()))]));

        assert_eq!(muxer.codec().encodes, 1);
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
}

use std::collections::BTreeMap;
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
}

#[derive(Debug)]
struct Viewer {
    view: View,
    chunks: Vec<Bytes>,
}

/// A subject that crosses the edge of a tile, with the tile on the other side.
struct Crossing {
    subject: EntityId,
    other: TileKey,
}

impl<C: ReplicationCodec> Muxer<C> {
    #[must_use]
    pub fn new(codec: C) -> Self {
        Self {
            codec,
            tiles: BTreeMap::new(),
            viewers: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn codec(&self) -> &C {
        &self.codec
    }

    /// Takes in the changes of one tick of a world.
    pub fn publish(&mut self, deltas: &TileDeltas<C::Data>) {
        let mut removals: BTreeMap<ViewerId, Vec<EntityId>> = BTreeMap::new();
        for delta in &deltas.tiles {
            self.publish_tile(delta, &mut removals);
        }
        for (viewer, subjects) in removals {
            if let Some(state) = self.viewers.get_mut(&viewer) {
                state.chunks.push(self.codec.remove(&subjects));
            }
        }
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
                chunks.push(self.codec.remove(&removed));
            }
        }

        for tile in view.tiles() {
            if kept.is_some_and(|old| old.contains(tile)) {
                continue;
            }
            let stream = self.tiles.entry(tile).or_default();
            stream.viewers.insert(viewer);
            for (&subject, cached) in &mut stream.subjects {
                if Some(subject) != view.subject {
                    chunks.push(spawn_bundle(&mut self.codec, subject, cached));
                }
            }
        }

        let state = self.viewers.entry(viewer).or_insert_with(|| Viewer {
            view,
            chunks: Vec::new(),
        });
        state.view = view;
        state.chunks.append(&mut chunks);
    }

    /// Forgets `viewer` and the bytes it has not taken. It gets no removal, because its
    /// connection is gone or its client drops its entities by itself.
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
    }

    /// Hands out the bytes of each viewer that has some, in the order of [`ViewerId`]. The bytes
    /// of a viewer are in the order to append them to its connection.
    pub fn take_output(&mut self) -> impl Iterator<Item = ViewerBytes> + '_ {
        self.viewers
            .iter_mut()
            .filter(|(_, state)| !state.chunks.is_empty())
            .map(|(&viewer, state)| ViewerBytes {
                viewer,
                chunks: mem::take(&mut state.chunks),
            })
    }

    fn publish_tile(
        &mut self,
        delta: &TileDelta<C::Data>,
        removals: &mut BTreeMap<ViewerId, Vec<EntityId>>,
    ) {
        let stream = self.tiles.entry(delta.tile).or_default();
        let mut entered = Vec::new();
        let mut left = Vec::new();
        for SubjectEvent { subject, event } in &delta.events {
            let subject = *subject;
            match event {
                EntityEvent::Spawn(_) => {
                    stream.subjects.insert(subject, None);
                }
                EntityEvent::Enter { from } => {
                    stream.subjects.insert(subject, None);
                    entered.push(Crossing {
                        subject,
                        other: *from,
                    });
                }
                EntityEvent::Leave { to } => {
                    stream.subjects.remove(&subject);
                    left.push(Crossing {
                        subject,
                        other: *to,
                    });
                }
                EntityEvent::Despawn => {
                    stream.subjects.remove(&subject);
                }
                EntityEvent::Move(_) | EntityEvent::Data(_) => {
                    if let Some(cached) = stream.subjects.get_mut(&subject) {
                        *cached = None;
                    }
                }
            }
        }

        let blob = self.codec.encode(delta);
        for &viewer in &stream.viewers {
            let Some(state) = self.viewers.get_mut(&viewer) else {
                continue;
            };
            let own = state.view.subject;
            let gained: Vec<EntityId> = entered
                .iter()
                .filter(|crossing| {
                    Some(crossing.subject) != own && !state.view.contains(crossing.other)
                })
                .map(|crossing| crossing.subject)
                .collect();
            append_blob(&mut state.chunks, &blob, |subject| {
                Some(subject) == own || gained.contains(&subject)
            });
            for subject in gained {
                if let Some(cached) = stream.subjects.get_mut(&subject) {
                    state
                        .chunks
                        .push(spawn_bundle(&mut self.codec, subject, cached));
                }
            }
            for crossing in &left {
                if Some(crossing.subject) != own && !state.view.contains(crossing.other) {
                    removals.entry(viewer).or_default().push(crossing.subject);
                }
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
                .keys()
                .copied()
                .filter(|&subject| Some(subject) != own),
        );
        if stream.is_empty() {
            self.tiles.remove(&tile);
        }
    }
}

fn spawn_bundle<C: ReplicationCodec>(
    codec: &mut C,
    subject: EntityId,
    cached: &mut Option<Bytes>,
) -> Bytes {
    cached
        .get_or_insert_with(|| codec.spawn_bundle(subject))
        .clone()
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
    }

    impl ReplicationCodec for TextCodec {
        type Data = ();

        fn encode(&mut self, delta: &TileDelta<()>) -> TileBlob {
            self.encodes += 1;
            let mut text = String::new();
            let mut ranges = Vec::new();
            for SubjectEvent { subject, event } in &delta.events {
                let start = text.len();
                let token = match event {
                    EntityEvent::Spawn(()) => "spawn",
                    EntityEvent::Move(()) => "move",
                    EntityEvent::Data(()) => "data",
                    EntityEvent::Despawn => "despawn",
                    EntityEvent::Enter { .. } | EntityEvent::Leave { .. } => continue,
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

        fn spawn_bundle(&mut self, subject: EntityId) -> Bytes {
            Bytes::from(format!("bundle {};", subject.0))
        }

        fn remove(&mut self, subjects: &[EntityId]) -> Bytes {
            let ids: Vec<String> = subjects.iter().map(|id| id.0.to_string()).collect();
            Bytes::from(format!("remove {};", ids.join(",")))
        }
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
    fn each_tile_delta_is_encoded_once_and_its_bytes_are_shared_by_all_viewers() {
        let mut muxer = Muxer::new(TextCodec::default());
        for viewer in 1..=3 {
            muxer.subscribe(ViewerId(viewer), view(ORIGIN, None));
        }

        muxer.publish(&tick(ORIGIN, vec![event(9, EntityEvent::Spawn(()))]));

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

use std::collections::BTreeMap;
use std::num::NonZeroU32;

use crate::entity_id::{ViewerId, WorldId};
use crate::muxer::Muxer;
use crate::replication_codec::ReplicationCodec;
use crate::tile_delta::TileDeltas;
use crate::view::View;
use crate::viewer_bytes::ViewerBytes;

/// A call to the [`Muxer`] of one world, sent over a link.
#[non_exhaustive]
pub enum MuxerInput<C: ReplicationCodec> {
    /// Starts the muxer of `world` with a new codec and tiles `tile_width` wide. A world that
    /// is open already keeps its muxer.
    Open {
        world: WorldId,
        tile_width: NonZeroU32,
    },
    /// Drops the muxer of `world` and the bytes it has not handed out.
    Close {
        world: WorldId,
    },
    Publish {
        world: WorldId,
        deltas: TileDeltas<C::Data>,
    },
    Subscribe {
        world: WorldId,
        viewer: ViewerId,
        view: View,
    },
    Unsubscribe {
        world: WorldId,
        viewer: ViewerId,
    },
    Codec {
        world: WorldId,
        input: C::Input,
    },
}

/// The [`Muxer`]s of many worlds. It applies each input to the muxer of its world, and an input
/// for a world that is not open does nothing.
pub struct Muxers<C: ReplicationCodec> {
    worlds: BTreeMap<WorldId, Muxer<C>>,
}

impl<C: ReplicationCodec> Default for Muxers<C> {
    fn default() -> Self {
        Self {
            worlds: BTreeMap::new(),
        }
    }
}

impl<C: ReplicationCodec + Default> Muxers<C> {
    pub fn apply(&mut self, input: MuxerInput<C>) {
        let world = match input {
            MuxerInput::Open { world, tile_width } => {
                self.worlds
                    .entry(world)
                    .or_insert_with(|| Muxer::new(C::default(), tile_width));
                return;
            }
            MuxerInput::Close { world } => {
                self.worlds.remove(&world);
                return;
            }
            MuxerInput::Publish { world, .. }
            | MuxerInput::Subscribe { world, .. }
            | MuxerInput::Unsubscribe { world, .. }
            | MuxerInput::Codec { world, .. } => world,
        };
        let Some(muxer) = self.worlds.get_mut(&world) else {
            return;
        };
        match input {
            MuxerInput::Publish { deltas, .. } => muxer.publish(&deltas),
            MuxerInput::Subscribe { viewer, view, .. } => muxer.subscribe(viewer, view),
            MuxerInput::Unsubscribe { viewer, .. } => muxer.unsubscribe(viewer),
            MuxerInput::Codec { input, .. } => muxer.codec_mut().receive(input),
            MuxerInput::Open { .. } | MuxerInput::Close { .. } => {}
        }
    }

    /// Hands out the bytes of each viewer, world by world in the order of [`WorldId`], as
    /// [`Muxer::take_output`] does.
    pub fn take_output(&mut self) -> impl Iterator<Item = ViewerBytes> + '_ {
        self.worlds.values_mut().flat_map(Muxer::take_output)
    }
}

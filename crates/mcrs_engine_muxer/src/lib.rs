//! The entity side of the Muxer. It takes the tile deltas of a world, keeps a stream for each
//! tile, and turns the subscriptions of the viewers into bytes that it encodes once and shares.

// std: mcrs_engine_link

mod entity_event;
mod entity_id;
mod muxer;
mod muxer_stage;
mod replication_codec;
mod tile_blob;
mod tile_delta;
mod tile_grid;
mod tile_key;
mod tile_radius;
mod tile_stream;
mod view;
mod viewer_bytes;

pub use entity_event::{EntityEvent, SubjectEvent};
pub use entity_id::{EntityId, ViewerId, WorldId};
pub use muxer::Muxer;
pub use muxer_stage::{MuxerInput, MuxerStage};
pub use replication_codec::ReplicationCodec;
pub use tile_blob::{SubjectRange, TileBlob};
pub use tile_delta::{TileDelta, TileDeltas};
pub use tile_grid::TileGrid;
pub use tile_key::TileKey;
pub use tile_radius::TileRadius;
pub use view::View;
pub use viewer_bytes::ViewerBytes;

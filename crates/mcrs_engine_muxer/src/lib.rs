//! The Muxer of the engine. Its entity side takes the tile deltas of a world, keeps a stream for
//! each tile, and turns the subscriptions of the viewers into bytes that it encodes once and
//! shares. Each viewer receives the tiles near it at a fast tier and the far ones at a slow tier,
//! at most a cap of subjects, and within a budget of bytes. Its chunk side keeps the version
//! stream of each chunk that a viewer holds or waits for.

// std: mcrs_engine_link

mod chunk_stream;
mod entity_event;
mod entity_id;
mod muxer;
mod muxer_settings;
mod muxer_stage;
mod planner;
mod replication_codec;
mod subscription;
mod tier;
mod tile_blob;
mod tile_delta;
mod tile_grid;
mod tile_key;
mod tile_radius;
mod tile_stream;
mod view;
mod viewer;
mod viewer_bytes;

pub use chunk_stream::{ChunkStream, StreamNeed, StreamVersion};
pub use entity_event::{EntityEvent, SubjectEvent};
pub use entity_id::{EntityId, ViewerId, WorldId};
pub use muxer::Muxer;
pub use muxer_settings::MuxerSettings;
pub use muxer_stage::{MuxerInput, MuxerStage};
pub use replication_codec::{BlobKind, Hold, ReplicationCodec};
pub use tier::Tier;
pub use tile_blob::{SubjectRange, TileBlob};
pub use tile_delta::{TileDelta, TileDeltas};
pub use tile_grid::TileGrid;
pub use tile_key::TileKey;
pub use tile_radius::TileRadius;
pub use view::View;
pub use viewer_bytes::ViewerBytes;

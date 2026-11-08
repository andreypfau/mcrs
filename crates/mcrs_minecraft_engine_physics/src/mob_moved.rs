use mcrs_engine_muxer::EntityId;
use mcrs_engine_muxer::TileKey;
use mcrs_minecraft_replication::MobState;

/// A living row whose state that the viewers see changed in the pass.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MobMoved {
    pub(crate) id: EntityId,
    pub(crate) state: MobState,
}

/// A row that the tile moves took from the tile `from` to the tile `to`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Crossing {
    pub(crate) id: EntityId,
    pub(crate) from: TileKey,
    pub(crate) to: TileKey,
}

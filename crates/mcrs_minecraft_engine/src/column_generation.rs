//! The generation of the columns of a world. The host runs one generation stage for each world,
//! for all its cells and for hot and cold space alike, so a column and the lower levels around
//! it are generated once. A cell asks for a column in the output of a tick and gets it in the
//! input of a later tick, as an answer of its data layer.

use bevy_ecs::entity::Entity;
use mcrs_engine_cell::CellId;
use mcrs_engine_store::{ChunkVersion, StoreOutput, VersionedChunk};
use mcrs_voxel::ColumnPos;

use crate::column_store::ColumnStoreOutput;
use mcrs_minecraft_region_disk::stored_column::StoredColumn;

/// A cell of the world of `label` asks the generation of the world for `column`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GenerationAsk {
    pub label: Entity,
    pub cell: CellId,
    pub column: ColumnPos,
}

/// What the generation of a world made of `column`: its blocks, unlit, or `None` when the
/// generation dropped the column, because it left every view.
#[derive(Clone, Debug)]
pub struct GenerationAnswer {
    pub column: ColumnPos,
    pub stored: Option<StoredColumn>,
}

/// A generated column lands as an answer of the data layer at version zero: the world never
/// wrote it.
impl From<GenerationAnswer> for ColumnStoreOutput {
    fn from(answer: GenerationAnswer) -> Self {
        let key = answer.column;
        match answer.stored {
            Some(chunk) => StoreOutput::Found {
                key,
                chunk: VersionedChunk {
                    version: ChunkVersion::default(),
                    chunk,
                },
            },
            None => StoreOutput::Missing { key },
        }
    }
}

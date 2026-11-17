use bevy_ecs::entity::Entity;
use mcrs_engine_cell::CellId;
use mcrs_engine_store::{ChunkVersion, StoreOutput, VersionedChunk};
use mcrs_voxel::ColumnPos;

use crate::column_store::ColumnStoreOutput;
use mcrs_minecraft_region_disk::stored_column::StoredColumn;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GenerationAsk {
    pub label: Entity,
    pub cell: CellId,
    pub column: ColumnPos,
}

#[derive(Clone, Debug)]
pub struct GenerationAnswer {
    pub column: ColumnPos,
    pub stored: Option<StoredColumn>,
}

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

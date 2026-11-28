use mcrs_minecraft_anvil::{BlendingData, ChunkStructures, RetroGen, UpgradeData};
use mcrs_minecraft_nbt::compound::NbtCompound;

pub use crate::cold_columns::SectionChanged;
pub use crate::stored_columns_landing::{PendingColumnFacts, TakenFromStore};

#[derive(Clone, Debug, Default, bevy_ecs::component::Component)]
pub struct RawBlockEntities(pub Vec<NbtCompound>);

#[derive(Clone, Debug, Default, bevy_ecs::component::Component)]
pub struct ColumnStructures(pub ChunkStructures);

#[derive(Clone, Debug, Default, bevy_ecs::component::Component)]
pub struct ColumnRecord {
    pub inhabited_time: i64,
    pub last_update: i64,
    pub retrogen: Option<RetroGen>,
    pub post_processing: Vec<Vec<i16>>,
    pub upgrade_data: UpgradeData,
    pub blending_data: Option<BlendingData>,
}

use mcrs_minecraft_anvil::{BlendingData, ChunkStructures, RetroGen, UpgradeData};
use mcrs_minecraft_nbt::compound::NbtCompound;

pub use crate::cold_columns::SectionChanged;
pub use crate::stored_columns_landing::{PendingColumnFacts, TakenFromStore};

/// Saved block entities whose id has no typed form. They stay beside the typed block entities
/// of their section and go back to the save unchanged.
#[derive(Clone, Debug, Default, bevy_ecs::component::Component)]
pub struct RawBlockEntities(pub Vec<NbtCompound>);

/// The structure starts and references of a saved column, as the save carries them.
#[derive(Clone, Debug, Default, bevy_ecs::component::Component)]
pub struct ColumnStructures(pub ChunkStructures);

/// The facts of a saved column that the engine does not simulate and writes back as it read
/// them.
#[derive(Clone, Debug, Default, bevy_ecs::component::Component)]
pub struct ColumnRecord {
    pub inhabited_time: i64,
    pub last_update: i64,
    pub retrogen: Option<RetroGen>,
    pub post_processing: Vec<Vec<i16>>,
    pub upgrade_data: UpgradeData,
    pub blending_data: Option<BlendingData>,
}

use mcrs_minecraft_nbt::compound::NbtCompound;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpawnData {
    pub entity: NbtCompound,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_spawn_rules: Option<NbtCompound>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub equipment: Option<NbtCompound>,
}

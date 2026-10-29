use mcrs_minecraft_nbt::compound::NbtCompound;
use serde::{Deserialize, Serialize};

pub fn one() -> i32 {
    1
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedItem {
    pub id: String,
    #[serde(default = "one")]
    pub count: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub components: Option<NbtCompound>,
}

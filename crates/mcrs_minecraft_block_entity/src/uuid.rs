use mcrs_minecraft_nbt::nbt_int_array;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Uuid(#[serde(serialize_with = "nbt_int_array")] pub [i32; 4]);

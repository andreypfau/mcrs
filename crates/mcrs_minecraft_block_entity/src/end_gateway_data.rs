use mcrs_minecraft_nbt::{nbt_flag, nbt_int_array};
use serde::{Deserialize, Serialize, Serializer};

fn exit_portal_array<S: Serializer>(
    exit_portal: &Option<[i32; 3]>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    match exit_portal {
        Some(position) => nbt_int_array(&position[..], serializer),
        None => serializer.serialize_none(),
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EndGatewayData {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    #[serde(rename = "Age", default)]
    pub age: i64,
    #[serde(
        rename = "exit_portal",
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "exit_portal_array"
    )]
    pub exit_portal: Option<[i32; 3]>,
    #[serde(
        rename = "ExactTeleport",
        default,
        skip_serializing_if = "std::ops::Not::not",
        deserialize_with = "nbt_flag"
    )]
    pub exact_teleport: bool,
}

use serde::{Deserialize, Serialize};

/// One entry of the player ban file, without the time of its creation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerBan {
    pub uuid: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

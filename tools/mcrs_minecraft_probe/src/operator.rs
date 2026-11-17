use mcrs_minecraft_protocol::uuid::Builder;
use md5::Digest;
use md5::Md5;
use serde::Serialize;

const OWNER_LEVEL: u8 = 4;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Operator {
    uuid: String,
    name: String,
    level: u8,
    bypasses_player_limit: bool,
}

impl Operator {
    pub(crate) fn new(name: &str) -> Self {
        Self {
            uuid: offline_profile_id(name),
            name: name.to_owned(),
            level: OWNER_LEVEL,
            bypasses_player_limit: false,
        }
    }
}

pub(crate) fn offline_profile_id(name: &str) -> String {
    let digest = Md5::digest(format!("OfflinePlayer:{name}").as_bytes());
    Builder::from_md5_bytes(digest.into())
        .into_uuid()
        .to_string()
}

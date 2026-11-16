use mcrs_minecraft_protocol::uuid::Builder;
use md5::Digest;
use md5::Md5;
use serde::Serialize;

const OWNER_LEVEL: u8 = 4;

/// One entry of the operator file of the vanilla server.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Operator {
    uuid: String,
    name: String,
    level: u8,
    bypasses_player_limit: bool,
}

impl Operator {
    /// The console command `op` asks the account service for the name, so the file is written
    /// with the profile id that a server in offline mode gives the player.
    pub(crate) fn new(name: &str) -> Self {
        Self {
            uuid: offline_profile_id(name),
            name: name.to_owned(),
            level: OWNER_LEVEL,
            bypasses_player_limit: false,
        }
    }
}

/// The profile id that a server in offline mode gives the player `name`, in its text form.
pub(crate) fn offline_profile_id(name: &str) -> String {
    let digest = Md5::digest(format!("OfflinePlayer:{name}").as_bytes());
    Builder::from_md5_bytes(digest.into())
        .into_uuid()
        .to_string()
}

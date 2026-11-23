use mcrs_minecraft_protocol::uuid::{Builder, Uuid};
use md5::{Digest, Md5};

#[must_use]
pub fn offline_player_uuid(username: &str) -> Uuid {
    let digest = Md5::digest(format!("OfflinePlayer:{username}").as_bytes());
    Builder::from_md5_bytes(digest.into()).into_uuid()
}

use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Operator {
    uuid: String,
    name: String,
    level: u8,
    bypasses_player_limit: bool,
}

impl Operator {
    pub(crate) fn new(name: &str, level: u8) -> Self {
        Self {
            uuid: offline_profile_id(name),
            name: name.to_owned(),
            level,
            bypasses_player_limit: false,
        }
    }
}

pub(crate) fn offline_profile_id(name: &str) -> String {
    mcrs_minecraft_probe::offline_player_uuid(name).to_string()
}

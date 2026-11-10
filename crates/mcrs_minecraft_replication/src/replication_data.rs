use crate::mob_report::MobReport;
use crate::player_data::PlayerData;

/// What a world publishes about a subject in a tile delta. One muxer link has one codec, so the
/// players and the mobs share this type. The codec ignores a variant on an event that does not
/// carry it.
#[derive(Clone, Debug, PartialEq)]
pub enum ReplicationData {
    Player(PlayerData),
    Mob(MobReport),
}

impl From<PlayerData> for ReplicationData {
    fn from(data: PlayerData) -> Self {
        Self::Player(data)
    }
}

impl From<MobReport> for ReplicationData {
    fn from(report: MobReport) -> Self {
        Self::Mob(report)
    }
}

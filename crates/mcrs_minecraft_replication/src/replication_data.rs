use crate::mob_report::MobReport;
use crate::player_data::PlayerData;

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

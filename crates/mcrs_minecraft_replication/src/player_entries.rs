use mcrs_minecraft_pose::Pose;
use mcrs_minecraft_protocol_game::entity::{MetaDataValue, MetadataEntry};
use mcrs_minecraft_protocol_game::entity_data_index::{POSE, SHARED_FLAGS};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct PlayerEntries {
    pub shared_flags: u8,
    pub pose: Pose,
}

impl PlayerEntries {
    #[must_use]
    pub fn changed_from(&self, sent: &Self) -> Vec<MetadataEntry<'static>> {
        let mut entries = Vec::new();
        if self.shared_flags != sent.shared_flags {
            entries.push(MetadataEntry {
                index: SHARED_FLAGS,
                value: MetaDataValue::Byte(self.shared_flags.cast_signed()),
            });
        }
        if self.pose != sent.pose {
            entries.push(MetadataEntry {
                index: POSE,
                value: MetaDataValue::Pose(self.pose),
            });
        }
        entries
    }
}

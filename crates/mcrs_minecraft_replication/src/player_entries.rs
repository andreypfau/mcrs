use mcrs_minecraft_pose::Pose;
use mcrs_minecraft_protocol_game::entity::{MetaDataValue, MetadataEntry};
use mcrs_minecraft_protocol_game::entity_data_index::{POSE, SHARED_FLAGS};

use crate::mob_bundle::health_entry;

const DEFAULT_HEALTH: f32 = 1.0;

#[derive(Clone, Copy, Debug)]
pub struct PlayerEntries {
    pub shared_flags: u8,
    pub pose: Pose,
    pub health: f32,
}

impl Default for PlayerEntries {
    fn default() -> Self {
        Self {
            shared_flags: 0,
            pose: Pose::default(),
            health: DEFAULT_HEALTH,
        }
    }
}

impl PartialEq for PlayerEntries {
    fn eq(&self, other: &Self) -> bool {
        self.shared_flags == other.shared_flags
            && self.pose == other.pose
            && self.health.to_bits() == other.health.to_bits()
    }
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
        if self.health.to_bits() != sent.health.to_bits() {
            entries.push(health_entry(self.health));
        }
        entries
    }
}

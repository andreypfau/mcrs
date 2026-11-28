use bytes::{Bytes, BytesMut};
use mcrs_minecraft_pose::Pose;
use mcrs_minecraft_protocol::{Result, VarInt, write_record};
use mcrs_minecraft_protocol_game::clientbound::ClientboundSetEntityData;
use mcrs_minecraft_protocol_game::entity::{MetaDataValue, Metadata, MetadataEntry};
use mcrs_minecraft_protocol_game::entity_data_index::{
    HEALTH, MOB_FLAG_LEFT_HANDED, MOB_FLAG_NO_AI, MOB_FLAGS, POSE,
};

use crate::encode::record;
use crate::mob_state::MobState;
use crate::subject_attributes::SubjectAttributes;
use mcrs_minecraft_protocol_game::ProtoStack;
use mcrs_minecraft_protocol_game::entity::MobLooks;
use mcrs_minecraft_registries::Wire;
use mcrs_voxel::BlockPos;

const ITEM_STACK: u8 = 8;

const FALLING_BLOCK_START: u8 = 8;

const NO_START: BlockPos = BlockPos::new(0, 0, 0);

#[derive(Clone, Copy, Debug)]
pub(crate) struct LivingEntries {
    pub(crate) health: f32,
    pub(crate) dying: bool,
}

impl LivingEntries {
    pub(crate) fn of(state: &MobState) -> Self {
        Self {
            health: state.health,
            dying: state.dying,
        }
    }
}

impl PartialEq for LivingEntries {
    fn eq(&self, other: &Self) -> bool {
        self.health.to_bits() == other.health.to_bits() && self.dying == other.dying
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Entries {
    Living(LivingEntries),
    Item(ProtoStack<Wire>),
    FallingBlock(BlockPos),
    Custom(Vec<MetadataEntry<'static>>),
}

impl Entries {
    pub(crate) fn custom(entries: &[MetadataEntry<'static>]) -> Self {
        let mut custom = Self::Custom(Vec::with_capacity(entries.len()));
        custom.merge(entries);
        custom
    }

    pub(crate) fn merge(&mut self, entries: &[MetadataEntry<'static>]) {
        let Self::Custom(custom) = self else {
            return;
        };
        for entry in entries {
            match custom.binary_search_by_key(&entry.index, |held| held.index) {
                Ok(at) => {
                    if let Some(held) = custom.get_mut(at) {
                        held.clone_from(entry);
                    }
                }
                Err(at) => custom.insert(at, entry.clone()),
            }
        }
    }

    pub(crate) fn all(&self) -> Vec<MetadataEntry<'static>> {
        match self {
            Self::Living(living) => {
                let mut entries = Vec::new();
                if living.dying {
                    entries.push(pose_entry());
                }
                entries.push(health_entry(living.health));
                entries
            }
            Self::Item(stack) => vec![item_entry(stack)],
            Self::FallingBlock(start) => vec![start_entry(*start)],
            Self::Custom(entries) => entries.clone(),
        }
    }

    pub(crate) fn changed_from(&self, sent: &Self) -> Vec<MetadataEntry<'static>> {
        match (self, sent) {
            (Self::Living(now), Self::Living(sent)) => {
                let mut changed = Vec::new();
                if now.dying != sent.dying {
                    changed.push(pose_entry());
                }
                if now.health.to_bits() != sent.health.to_bits() {
                    changed.push(health_entry(now.health));
                }
                changed
            }
            (Self::Custom(now), Self::Custom(sent)) => now
                .iter()
                .filter(|entry| !sent.contains(entry))
                .cloned()
                .collect(),
            _ => self.all(),
        }
    }
}

#[must_use]
pub(crate) fn spawn_data(looks: &MobLooks, living: LivingEntries) -> Metadata<'static> {
    let mut entries = Vec::with_capacity(3 + looks.type_entries.len());
    if living.dying {
        entries.push(pose_entry());
    }
    entries.push(health_entry(living.health));
    let flags = if looks.no_ai { MOB_FLAG_NO_AI } else { 0 }
        | if looks.left_handed {
            MOB_FLAG_LEFT_HANDED
        } else {
            0
        };
    if flags != 0 {
        entries.push(MetadataEntry {
            index: MOB_FLAGS,
            value: MetaDataValue::Byte(flags),
        });
    }
    entries.extend(looks.type_entries.iter().cloned());
    Metadata(entries)
}

pub(crate) fn pose_entry() -> MetadataEntry<'static> {
    MetadataEntry {
        index: POSE,
        value: MetaDataValue::Pose(Pose::Dying),
    }
}

pub(crate) fn health_entry(health: f32) -> MetadataEntry<'static> {
    MetadataEntry {
        index: HEALTH,
        value: MetaDataValue::Float(health),
    }
}

fn item_entry(stack: &ProtoStack<Wire>) -> MetadataEntry<'static> {
    MetadataEntry {
        index: ITEM_STACK,
        value: MetaDataValue::Slot(stack.clone()),
    }
}

fn start_entry(start: BlockPos) -> MetadataEntry<'static> {
    MetadataEntry {
        index: FALLING_BLOCK_START,
        value: MetaDataValue::BlockPos(start),
    }
}

pub(crate) fn spawn_extras(
    id: VarInt,
    looks: &MobLooks,
    entries: &Entries,
    attributes: &SubjectAttributes,
) -> Result<Bytes> {
    let living = match entries {
        Entries::Living(living) => *living,
        Entries::Item(stack) => {
            return record(&ClientboundSetEntityData {
                entity_id: id,
                metadata: Metadata(vec![item_entry(stack)]),
            });
        }
        Entries::FallingBlock(NO_START) => return Ok(Bytes::new()),
        Entries::FallingBlock(start) => {
            return record(&ClientboundSetEntityData {
                entity_id: id,
                metadata: Metadata(vec![start_entry(*start)]),
            });
        }
        Entries::Custom(entries) if entries.is_empty() => return Ok(Bytes::new()),
        Entries::Custom(entries) => {
            return record(&ClientboundSetEntityData {
                entity_id: id,
                metadata: Metadata(entries.clone()),
            });
        }
    };
    let mut bytes = BytesMut::new();
    write_record(
        &mut bytes,
        &ClientboundSetEntityData {
            entity_id: id,
            metadata: spawn_data(looks, living),
        },
    )?;
    attributes.write_spawn(id, &mut bytes)?;
    Ok(bytes.freeze())
}

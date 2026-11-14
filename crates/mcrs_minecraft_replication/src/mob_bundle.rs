use bytes::{Bytes, BytesMut};
use mcrs_minecraft_entity::Pose;
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

/// The index of the stack in the entity data of an item entity.
const ITEM_STACK: u8 = 8;

/// The index of the start position in the entity data of a falling block (FBK-DAT-003).
const FALLING_BLOCK_START: u8 = 8;

/// The default start position of a falling block (FBK-DAT-003).
const NO_START: BlockPos = BlockPos::new(0, 0, 0);

/// The living entries of a mob that change in its life.
#[derive(Clone, Copy, Debug)]
pub(crate) struct LivingEntries {
    pub(crate) health: f32,
    /// The pose is `dying`.
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
    /// Compares the bits of the health, as the entry on the wire does.
    fn eq(&self, other: &Self) -> bool {
        self.health.to_bits() == other.health.to_bits() && self.dying == other.dying
    }
}

/// The entity data entries of a tracked entity that change in its life.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Entries {
    Living(LivingEntries),
    /// The stack of an item entity (IEN-NET-003).
    Item(ProtoStack<Wire>),
    /// The start position of a falling block.
    FallingBlock(BlockPos),
    /// The entries of a custom kind, one for each index, in the order of the indices.
    Custom(Vec<MetadataEntry<'static>>),
}

impl Entries {
    /// The entries of a custom kind; a later entry replaces an earlier one of the same index.
    pub(crate) fn custom(entries: &[MetadataEntry<'static>]) -> Self {
        let mut custom = Self::Custom(Vec::with_capacity(entries.len()));
        custom.merge(entries);
        custom
    }

    /// Replaces the entries of a custom kind that have the index of an entry of `entries`, and
    /// adds the others in the order of the indices. Does nothing for another kind.
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

    /// Every entry, for a client that missed updates.
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

    /// The entries that differ from `sent`.
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

/// The entity data of a mob that differ from the defaults, in the order of the indices
/// (TRK-DATA-002, TRK-DATA-006): its pose, its health, its mob flags and the entries of its type.
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

/// The packets of the spawn bundle of the entity `id` after its `add_entity`, as records. A mob
/// has its `set_entity_data` and, when an instance differs from that of a new mob of its type, its
/// `update_attributes` (TRK-DATA-004). An item entity has the `set_entity_data` with its stack
/// (IEN-NET-001), and a falling block the one with its start position when it is not the default
/// (FBK-SPN-002). A custom kind has the `set_entity_data` with its entries when it has one.
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

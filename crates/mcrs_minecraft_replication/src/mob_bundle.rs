use bytes::{Bytes, BytesMut};
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_entity::attribute_keys::Attribute;
use mcrs_minecraft_protocol::{Result, VarInt, write_record};
use mcrs_minecraft_protocol_game::clientbound::{
    AttributeSnapshot, ClientboundSetEntityData, ClientboundUpdateAttributes,
};
use mcrs_minecraft_protocol_game::entity::{MetaDataValue, Metadata, MetadataEntry, Pose};
use mcrs_minecraft_protocol_game::entity_data_index::{
    HEALTH, MOB_FLAG_LEFT_HANDED, MOB_FLAG_NO_AI, MOB_FLAGS, POSE,
};

use crate::mob_looks::MobLooks;
use crate::mob_state::MobState;

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

/// The `set_entity_data` and the `update_attributes` of the spawn bundle of the mob `id`, as
/// records. The attributes are the list that the reference sends: `movement_speed` with its base
/// and no modifier (TRK-DATA-004).
pub(crate) fn spawn_extras(
    id: VarInt,
    kind: &EntityTypeData,
    looks: &MobLooks,
    living: LivingEntries,
) -> Result<Bytes> {
    let movement_speed = kind
        .base_attributes
        .map_or(Attribute::MovementSpeed.definition().default, |base| {
            base.movement_speed
        });
    let mut bytes = BytesMut::new();
    write_record(
        &mut bytes,
        &ClientboundSetEntityData {
            entity_id: id,
            metadata: spawn_data(looks, living),
        },
    )?;
    write_record(
        &mut bytes,
        &ClientboundUpdateAttributes {
            entity_id: id,
            attributes: vec![AttributeSnapshot {
                attribute: Attribute::MovementSpeed,
                base: movement_speed,
                modifiers: Vec::new(),
            }],
        },
    )?;
    Ok(bytes.freeze())
}

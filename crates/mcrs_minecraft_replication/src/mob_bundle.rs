use bytes::{Bytes, BytesMut};
use mcrs_minecraft_entity::attribute_keys::Attribute;
use mcrs_minecraft_mob::MobType;
use mcrs_minecraft_protocol::{Result, VarInt, write_record};
use mcrs_minecraft_protocol_game::clientbound::{
    AttributeSnapshot, ClientboundSetEntityData, ClientboundUpdateAttributes,
};
use mcrs_minecraft_protocol_game::entity::{MetaDataValue, Metadata, MetadataEntry, Pose};

use crate::mob_looks::{MobLooks, MobVariant};

const POSE: u8 = 6;
const HEALTH: u8 = 9;
const MOB_FLAGS: u8 = 15;
const NO_AI: i8 = 1;
const LEFT_HANDED: i8 = 2;

/// The entity data of a mob that differ from the defaults, in the order of the indices
/// (TRK-DATA-002, TRK-DATA-006): its pose, its health, its mob flags and its variant entries.
#[must_use]
pub(crate) fn spawn_data(looks: &MobLooks, health: f32, dying: bool) -> Metadata<'static> {
    let mut entries = Vec::new();
    if dying {
        entries.push(pose_entry());
    }
    entries.push(health_entry(health));
    let flags =
        if looks.no_ai { NO_AI } else { 0 } | if looks.left_handed { LEFT_HANDED } else { 0 };
    if flags != 0 {
        entries.push(MetadataEntry {
            index: MOB_FLAGS,
            value: MetaDataValue::Byte(flags),
        });
    }
    let mut entry = |index: u8, value: Option<MetaDataValue<'static>>| {
        entries.extend(value.map(|value| MetadataEntry { index, value }));
    };
    match looks.variant {
        MobVariant::Plain => {}
        MobVariant::Pig { variant, sound } => {
            entry(19, variant.map(MetaDataValue::PigVariant));
            entry(20, sound.map(MetaDataValue::PigSoundVariant));
        }
        MobVariant::Cow { variant, sound } => {
            entry(18, variant.map(MetaDataValue::CowVariant));
            entry(19, sound.map(MetaDataValue::CowSoundVariant));
        }
        MobVariant::Chicken { variant, sound } => {
            entry(18, variant.map(MetaDataValue::ChickenVariant));
            entry(19, sound.map(MetaDataValue::ChickenSoundVariant));
        }
        MobVariant::Sheep { wool } => {
            entry(18, (wool != 0).then_some(MetaDataValue::Byte(wool as i8)));
        }
    }
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
    kind: MobType,
    looks: &MobLooks,
    health: f32,
    dying: bool,
) -> Result<Bytes> {
    let mut bytes = BytesMut::new();
    write_record(
        &mut bytes,
        &ClientboundSetEntityData {
            entity_id: id,
            metadata: spawn_data(looks, health, dying),
        },
    )?;
    write_record(
        &mut bytes,
        &ClientboundUpdateAttributes {
            entity_id: id,
            attributes: vec![AttributeSnapshot {
                attribute: Attribute::MovementSpeed,
                base: kind.movement_speed(),
                modifiers: Vec::new(),
            }],
        },
    )?;
    Ok(bytes.freeze())
}

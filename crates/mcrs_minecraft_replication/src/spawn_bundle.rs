use crate::delta_base::DeltaBase;
use crate::encode::record;
use bytes::{Bytes, BytesMut};
use glam::DVec3;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundBundleDelimiter;
use mcrs_minecraft_protocol::{Result, VarInt};
use mcrs_minecraft_protocol_game::LpVec3;
use mcrs_minecraft_protocol_game::clientbound::ClientboundAddEntity;
use uuid::Uuid;

/// A subject as a client sees it when the subject appears.
#[derive(Clone, Copy, Debug)]
pub struct SpawnSubject<'a> {
    pub id: VarInt,
    pub uuid: Uuid,
    pub kind: EntityType,
    pub base: &'a DeltaBase,
}

/// The delimiter that opens and closes a bundle. The client applies the packets between two
/// delimiters in one tick.
pub fn bundle_delimiter() -> Result<Bytes> {
    record(&ClientboundBundleDelimiter)
}

/// The end of the spawn bundle of `subject`: its spawn, then the delimiter that closes the
/// bundle. A spawn bundle is the delimiter, the profile entry of a player when its client does
/// not have the entry, and this end. The profile entry comes first because the client does not
/// create a player without it. The end is the same for every viewer.
pub fn spawn_bundle_end(subject: &SpawnSubject<'_>) -> Result<Bytes> {
    let spawn = record(&ClientboundAddEntity {
        id: subject.id,
        uuid: subject.uuid,
        kind: subject.kind,
        pos: subject.base.position,
        movement: LpVec3(DVec3::ZERO),
        pitch: subject.base.pitch,
        yaw: subject.base.yaw,
        head_yaw: subject.base.head_yaw,
        data: VarInt(0),
    })?;
    let delimiter = bundle_delimiter()?;
    let mut end = BytesMut::with_capacity(spawn.len().saturating_add(delimiter.len()));
    end.extend_from_slice(&spawn);
    end.extend_from_slice(&delimiter);
    Ok(end.freeze())
}

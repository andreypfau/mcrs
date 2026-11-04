use crate::delta_base::DeltaBase;
use crate::encode::encode;
use bytes::Bytes;
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

/// The packets that make a subject appear on a client, in wire order. The client applies them
/// in one tick, between the two delimiters. The profile entry of a player comes first: the
/// client does not create a player without it. Pass `None` when the client has the entry.
pub fn spawn_bundle(
    subject: &SpawnSubject<'_>,
    profile_entry: Option<&Bytes>,
) -> Result<Vec<Bytes>> {
    let mut packets = Vec::with_capacity(4);
    packets.push(encode(&ClientboundBundleDelimiter)?);
    if let Some(entry) = profile_entry {
        packets.push(entry.clone());
    }
    packets.push(encode(&ClientboundAddEntity {
        id: subject.id,
        uuid: subject.uuid,
        kind: subject.kind,
        pos: subject.base.position,
        movement: LpVec3(DVec3::ZERO),
        pitch: subject.base.pitch,
        yaw: subject.base.yaw,
        head_yaw: subject.base.head_yaw,
        data: VarInt(0),
    })?);
    packets.push(encode(&ClientboundBundleDelimiter)?);
    Ok(packets)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::delta_base::Pose;
    use mcrs_minecraft_core::Look;

    #[test]
    fn the_profile_entry_goes_between_the_delimiters_before_the_spawn() -> Result<()> {
        let base = DeltaBase::new(&Pose {
            position: DVec3::new(1.0, 64.0, 2.0),
            look: Look::default(),
        });
        let subject = SpawnSubject {
            id: VarInt(7),
            uuid: Uuid::nil(),
            kind: EntityType::Player,
            base: &base,
        };
        let entry = Bytes::from_static(b"entry");

        let with_entry = spawn_bundle(&subject, Some(&entry))?;
        let without_entry = spawn_bundle(&subject, None)?;

        let delimiter = encode(&ClientboundBundleDelimiter)?;
        assert_eq!(with_entry.len(), 4);
        assert_eq!(with_entry.first(), Some(&delimiter));
        assert_eq!(with_entry.get(1), Some(&entry));
        assert_eq!(with_entry.last(), Some(&delimiter));
        assert_eq!(without_entry.len(), 3);
        assert_eq!(without_entry.get(1), with_entry.get(2));
        Ok(())
    }
}

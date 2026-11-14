use bytes::BufMut;
use bytes::Bytes;
use bytes::BytesMut;
use mcrs_minecraft_entity::AttributeBases;
use mcrs_minecraft_entity::AttributeSet;
use mcrs_minecraft_entity::attribute_keys::Attribute;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_protocol::{Packet, Result, VarInt};
use mcrs_minecraft_protocol_game::clientbound::{AttributeSnapshot, ClientboundUpdateAttributes};

use crate::attribute_snapshot::attribute_snapshot;

/// The `update_attributes` packet with its id that gives the client of a player the instances of
/// `attributes` in `set`, about its own entity `id` (ATR-NET-002, MVC-PKT-031). An attribute
/// without an instance in `set` has the base of the type `player` and no modifier. `None` when
/// `attributes` is empty.
pub fn own_attributes(
    id: VarInt,
    set: &AttributeSet,
    attributes: impl IntoIterator<Item = Attribute>,
) -> Result<Option<Bytes>> {
    let bases = AttributeBases::of(EntityType::Player);
    let attributes: Vec<AttributeSnapshot<'_>> = attributes
        .into_iter()
        .map(|attribute| {
            let instance = set
                .instances()
                .find(|instance| instance.attribute() == attribute);
            attribute_snapshot(bases, attribute, instance)
        })
        .collect();
    if attributes.is_empty() {
        return Ok(None);
    }
    let mut packet = BytesMut::new().writer();
    ClientboundUpdateAttributes {
        entity_id: id,
        attributes,
    }
    .encode_with_id(&mut packet)?;
    Ok(Some(packet.into_inner().freeze()))
}

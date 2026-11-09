use mcrs_engine_muxer::EntityId;
use mcrs_minecraft_protocol::VarInt;

/// The id on the wire of `subject`: its low 32 bits. The id of a player is odd and the id of
/// any other entity is even, so the two kinds keep apart on the wire too.
#[must_use]
pub fn wire_id(subject: EntityId) -> VarInt {
    VarInt(subject.0 as u32 as i32)
}

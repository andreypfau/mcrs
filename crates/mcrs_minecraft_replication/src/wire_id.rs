use mcrs_engine_muxer::EntityId;
use mcrs_minecraft_protocol::VarInt;

#[must_use]
pub fn wire_id(subject: EntityId) -> VarInt {
    VarInt(subject.0 as u32 as i32)
}

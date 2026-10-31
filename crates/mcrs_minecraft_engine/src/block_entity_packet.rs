use mcrs_minecraft_block_entity::GeneratedBlockEntity;
use mcrs_minecraft_nbt::to_nbt_compound;
use mcrs_minecraft_protocol::chunk::ChunkDataBlockEntity;
use std::borrow::Cow;

/// The chunk packet's entry, carrying the same compound the save holds.
pub fn packet_entry(
    entry: &GeneratedBlockEntity,
) -> Result<ChunkDataBlockEntity<'static>, mcrs_minecraft_nbt::Error> {
    let pos = entry.position();
    Ok(ChunkDataBlockEntity {
        packed_xz: (((pos.x & 15) << 4) | (pos.z & 15)) as i8,
        y: pos.y as i16,
        kind: entry.kind(),
        data: Cow::Owned(to_nbt_compound(entry)?),
    })
}

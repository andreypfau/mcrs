use mcrs_minecraft_nbt::compound::NbtCompound;

/// The buffers that a match reads the host into. Keep one per caller and pass it to each match.
/// A match that reads no saved data of a block entity or an entity allocates nothing. A match
/// that reads saved data allocates for the tags that the host writes.
#[derive(Clone, Debug, Default)]
pub struct PredicateScratch {
    pub(crate) block_entity: NbtCompound,
    pub(crate) entity: NbtCompound,
    pub(crate) nested_matches: u32,
}

use mcrs_minecraft_nbt::compound::NbtCompound;

#[derive(Clone, Debug, Default)]
pub struct PredicateScratch {
    pub(crate) block_entity: NbtCompound,
    pub(crate) entity: NbtCompound,
    pub(crate) nested_matches: u32,
}

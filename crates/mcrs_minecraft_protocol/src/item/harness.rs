pub trait Sample: Sized {
    fn samples() -> Vec<Self>;

    /// The NBT tag id each dotted path below this sample's persistent form
    /// must carry, `""` naming the root; a width vanilla would write
    /// differently is a persistence bug the round trip alone cannot see.
    fn nbt_tags(&self) -> Vec<(&'static str, u8)> {
        Vec::new()
    }
}

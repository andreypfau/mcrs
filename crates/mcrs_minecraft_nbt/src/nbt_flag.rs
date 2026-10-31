/// NBT has no boolean, so a flag is a byte. A tagged enum buffers the compound
/// before it knows the variant, and a buffered byte never reaches
/// `deserialize_bool`, so a `bool` field inside one reads through this.
pub fn nbt_flag<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<bool, D::Error> {
    struct Flag;
    impl serde::de::Visitor<'_> for Flag {
        type Value = bool;

        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("a boolean or the byte standing for one")
        }

        fn visit_bool<E>(self, value: bool) -> Result<bool, E> {
            Ok(value)
        }

        fn visit_i64<E>(self, value: i64) -> Result<bool, E> {
            Ok(value != 0)
        }

        fn visit_u64<E>(self, value: u64) -> Result<bool, E> {
            Ok(value != 0)
        }
    }
    deserializer.deserialize_any(Flag)
}

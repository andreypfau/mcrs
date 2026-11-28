use serde::{Deserialize, Deserializer, de};

macro_rules! discriminators {
    ($($name:ident = $id:literal),* $(,)?) => {$(
        pub(crate) fn $name<'de, D: Deserializer<'de>>(d: D) -> Result<(), D::Error> {
            let id = String::deserialize(d)?;
            if id != $id {
                return Err(de::Error::custom(format_args!("Unknown element id: {id}")));
            }
            Ok(())
        }
    )*};
}

discriminators! {
    source_entity = "entity",
    source_block = "block",
    source_storage = "storage",
    object_atlas = "atlas",
    object_player = "player",
}

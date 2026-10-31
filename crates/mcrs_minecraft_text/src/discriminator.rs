use serde::{Deserialize, Deserializer, de};

/// The discriminator a variant writes under `source` / `object`: a marker
/// that rejects any other name, so the derived first-match implements
/// `StrictEither` for the untagged enums below.
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

use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::from_tag;
use mcrs_minecraft_nbt::tag::NbtTag;
use serde::{Deserialize, Deserializer, de};

/// A tagged map read without serde's `Content` buffer: the compound keeps
/// every NBT tag as it came, and the variant is picked from `action`.
pub(crate) fn tagged_compound<'de, D: Deserializer<'de>>(
    d: D,
    tag: &str,
) -> Result<(String, NbtCompound), D::Error> {
    let mut compound = NbtCompound::deserialize(d)?;
    let Some(index) = compound.child_tags.iter().position(|(key, _)| key == tag) else {
        return Err(de::Error::missing_field("action"));
    };
    match compound.child_tags.remove(index).1 {
        NbtTag::String(name) => Ok((name, compound)),
        _ => Err(de::Error::custom(format_args!("'{tag}' is not a string"))),
    }
}

pub(crate) fn variant<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    compound: NbtCompound,
) -> Result<T, D::Error> {
    from_tag(NbtTag::Compound(compound)).map_err(de::Error::custom)
}

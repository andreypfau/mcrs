use mcrs_minecraft_nbt::tag::NbtTag;

/// The text of a number argument of a translation, as the game writes it: an integer in decimal,
/// a float in the notation of Java. A tag that is not a number has no text.
#[must_use]
pub fn primitive_text(tag: &NbtTag) -> String {
    match tag {
        NbtTag::Byte(value) => value.to_string(),
        NbtTag::Short(value) => value.to_string(),
        NbtTag::Int(value) => value.to_string(),
        NbtTag::Long(value) => value.to_string(),
        NbtTag::Float(value) => mcrs_minecraft_snbt::java_float(*value),
        NbtTag::Double(value) => mcrs_minecraft_snbt::java_double(*value),
        _ => String::new(),
    }
}

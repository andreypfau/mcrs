use std::fmt::Write;

use mcrs_minecraft_nbt::tag::NbtTag;

#[must_use]
pub fn primitive_text(tag: &NbtTag) -> String {
    let mut out = String::new();
    write_primitive_text(tag, &mut out);
    out
}

pub fn write_primitive_text(tag: &NbtTag, out: &mut String) {
    match tag {
        NbtTag::Byte(value) => _ = write!(out, "{value}"),
        NbtTag::Short(value) => _ = write!(out, "{value}"),
        NbtTag::Int(value) => _ = write!(out, "{value}"),
        NbtTag::Long(value) => _ = write!(out, "{value}"),
        NbtTag::Float(value) => out.push_str(&mcrs_minecraft_snbt::java_float(*value)),
        NbtTag::Double(value) => out.push_str(&mcrs_minecraft_snbt::java_double(*value)),
        _ => {}
    }
}

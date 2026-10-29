use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write;

use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;

use crate::java_number::write_java_number;
use crate::text;

pub fn write(tag: &NbtTag) -> String {
    let mut out = String::new();
    write_tag(tag, &mut out);
    out
}

pub fn write_compound(compound: &NbtCompound) -> String {
    write(&NbtTag::Compound(compound.clone()))
}

fn write_tag(tag: &NbtTag, out: &mut String) {
    match tag {
        NbtTag::End => out.push_str("END"),
        NbtTag::Byte(v) => _ = write!(out, "{v}b"),
        NbtTag::Short(v) => _ = write!(out, "{v}s"),
        NbtTag::Int(v) => _ = write!(out, "{v}"),
        NbtTag::Long(v) => _ = write!(out, "{v}L"),
        NbtTag::Float(v) => _ = write_java_number(out, *v).and_then(|()| out.write_char('f')),
        NbtTag::Double(v) => _ = write_java_number(out, *v).and_then(|()| out.write_char('d')),
        NbtTag::String(v) => quote_and_escape(v, out),
        NbtTag::ByteArray(v) => {
            out.push_str("[B;");
            for (i, b) in v.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                _ = write!(out, "{}B", *b as i8);
            }
            out.push(']');
        }
        NbtTag::IntArray(v) => {
            out.push_str("[I;");
            for (i, n) in v.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                _ = write!(out, "{n}");
            }
            out.push(']');
        }
        NbtTag::LongArray(v) => {
            out.push_str("[L;");
            for (i, n) in v.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                _ = write!(out, "{n}L");
            }
            out.push(']');
        }
        NbtTag::List(v) => {
            out.push('[');
            for (i, tag) in v.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_tag(tag, out);
            }
            out.push(']');
        }
        NbtTag::Compound(v) => {
            let mut entries: Vec<&(String, NbtTag)> = v.child_tags.iter().collect();
            entries.sort_by(|a, b| a.0.encode_utf16().cmp(b.0.encode_utf16()));
            out.push('{');
            for (i, (key, value)) in entries.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                if is_unquoted_key(key) {
                    out.push_str(key);
                } else {
                    quote_and_escape(key, out);
                }
                out.push(':');
                write_tag(value, out);
            }
            out.push('}');
        }
    }
}

fn is_unquoted_key(key: &str) -> bool {
    let mut chars = key.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '.' || first == '_')
        && chars.all(text::is_unquoted)
        && !key.eq_ignore_ascii_case("true")
        && !key.eq_ignore_ascii_case("false")
}

fn quote_and_escape(input: &str, out: &mut String) {
    let quote = match input.chars().find(|c| matches!(c, '"' | '\'')) {
        Some('"') => '\'',
        _ => '"',
    };
    out.push(quote);
    for c in input.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' | '\'' => {
                if quote == c {
                    out.push('\\');
                }
                out.push(c);
            }
            '\u{8}' => out.push_str("\\b"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\u{c}' => out.push_str("\\f"),
            '\r' => out.push_str("\\r"),
            c if c < ' ' => _ = write!(out, "\\x{:02X}", c as u32),
            _ => out.push(c),
        }
    }
    out.push(quote);
}

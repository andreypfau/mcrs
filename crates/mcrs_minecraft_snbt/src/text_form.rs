use alloc::string::String;
use core::fmt::Display;
use core::fmt::Write;

use mcrs_minecraft_nbt::tag::NbtTag;

use crate::java_number::write_java_number;
use crate::text;
use crate::write::quote_and_escape;

const FOLD: &str = "<...>";
const FOLDED_LEVEL: usize = 64;
const SHOWN_ELEMENTS: usize = 128;

#[must_use]
pub fn text_form(tag: &NbtTag) -> String {
    let mut out = String::new();
    write_tag(tag, 0, &mut out);
    out
}

fn write_tag(tag: &NbtTag, level: usize, out: &mut String) {
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
            write_array(
                'B',
                v.iter().map(|byte| i8::from_ne_bytes([*byte])),
                "b",
                out,
            );
        }
        NbtTag::IntArray(v) => write_array('I', v.iter(), "", out),
        NbtTag::LongArray(v) => write_array('L', v.iter(), "L", out),
        NbtTag::List(v) if v.is_empty() => out.push_str("[]"),
        NbtTag::List(_) if level >= FOLDED_LEVEL => _ = write!(out, "[{FOLD}]"),
        NbtTag::List(v) => {
            out.push('[');
            for (index, element) in v.iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                write_tag(element, level.saturating_add(1), out);
            }
            out.push(']');
        }
        NbtTag::Compound(v) if v.is_empty() => out.push_str("{}"),
        NbtTag::Compound(_) if level >= FOLDED_LEVEL => _ = write!(out, "{{{FOLD}}}"),
        NbtTag::Compound(v) => {
            out.push('{');
            for (index, (key, value)) in v.child_tags.iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                if !key.is_empty() && key.chars().all(text::is_unquoted) {
                    out.push_str(key);
                } else {
                    quote_and_escape(key, out);
                }
                out.push_str(": ");
                write_tag(value, level.saturating_add(1), out);
            }
            out.push('}');
        }
    }
}

fn write_array<T: Display>(
    letter: char,
    elements: impl ExactSizeIterator<Item = T>,
    suffix: &str,
    out: &mut String,
) {
    let folded = elements.len() > SHOWN_ELEMENTS;
    let _ = write!(out, "[{letter};");
    for (index, element) in elements.take(SHOWN_ELEMENTS).enumerate() {
        if index > 0 {
            out.push(',');
        }
        let _ = write!(out, " {element}{suffix}");
    }
    if folded {
        let _ = write!(out, ",{FOLD}");
    }
    out.push(']');
}

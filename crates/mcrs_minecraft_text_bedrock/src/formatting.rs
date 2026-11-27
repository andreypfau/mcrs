use mcrs_minecraft_text::color::NamedColor;
use mcrs_minecraft_text::{Color, ResolvedStyle, TextTypes};

use crate::BedrockDrops;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct BedrockStyle {
    pub(crate) color: Option<NamedColor>,
    pub(crate) obfuscated: bool,
    pub(crate) bold: bool,
    pub(crate) italic: bool,
}

impl<I: TextTypes> From<ResolvedStyle<'_, I>> for BedrockStyle {
    fn from(style: ResolvedStyle<'_, I>) -> Self {
        Self {
            color: style.color.map(|color| match color {
                Color::Named(named) => named,
                Color::Rgb(rgb) => rgb.to_named_lossy(),
            }),
            obfuscated: style.obfuscated,
            bold: style.bold,
            italic: style.italic,
        }
    }
}

pub(crate) fn write_bedrock_change(out: &mut String, from: BedrockStyle, to: BedrockStyle) {
    let flags = |s: BedrockStyle| [(s.obfuscated, 'k'), (s.bold, 'l'), (s.italic, 'o')];

    let lost = flags(from)
        .iter()
        .zip(flags(to))
        .any(|(old, new)| old.0 && !new.0)
        || (from.color.is_some() && to.color.is_none());

    if lost {
        out.push_str("§r");
    }

    let cleared = lost || from.color != to.color;
    if cleared && let Some(color) = to.color {
        out.push('§');
        out.push(color.hex_digit());
    }

    for ((old, _), (new, code)) in flags(from).into_iter().zip(flags(to)) {
        if new && (cleared || !old) {
            out.push('§');
            out.push(code);
        }
    }
}

#[must_use]
pub(crate) fn rewrite_literal_text(input: &str, drops: &mut BedrockDrops) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '§' {
            if let Some(&next) = chars.peek() {
                match next {
                    'm' | 'M' => {
                        chars.next();
                        drops.strikethrough = drops.strikethrough.saturating_add(1);
                    }
                    'n' | 'N' => {
                        chars.next();
                        drops.underlined = drops.underlined.saturating_add(1);
                    }
                    _ => {
                        out.push('§');
                    }
                }
            } else {
                out.push('§');
            }
        } else {
            out.push(c);
        }
    }
    out
}

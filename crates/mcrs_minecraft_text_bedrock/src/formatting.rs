use mcrs_minecraft_text::color::NamedColor;
use mcrs_minecraft_text::{Color, Style, TextTypes};

use crate::BedrockDrops;

/// Active Bedrock style state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct BedrockStyle {
    pub(crate) color: Option<NamedColor>,
    pub(crate) obfuscated: bool,
    pub(crate) bold: bool,
    pub(crate) italic: bool,
}

impl BedrockStyle {
    /// Updates this style with the canonical style, recording dropped attributes.
    #[must_use]
    pub(crate) fn update_with<I: TextTypes>(
        self,
        style: &Style<I>,
        drops: &mut BedrockDrops,
    ) -> Self {
        if style.click_event.is_some() {
            drops.click_events = drops.click_events.saturating_add(1);
        }
        if style.hover_event.is_some() {
            drops.hover_events = drops.hover_events.saturating_add(1);
        }
        if style.font.is_some() {
            drops.fonts = drops.fonts.saturating_add(1);
        }
        if style.strikethrough == Some(true) {
            drops.strikethrough = drops.strikethrough.saturating_add(1);
        }
        if style.underlined == Some(true) {
            drops.underlined = drops.underlined.saturating_add(1);
        }

        let color = match style.color {
            Some(Color::Named(named)) => Some(named),
            Some(Color::Rgb(rgb)) => Some(rgb.to_named_lossy()),
            None => self.color,
        };

        Self {
            color,
            obfuscated: style.obfuscated.unwrap_or(self.obfuscated),
            bold: style.bold.unwrap_or(self.bold),
            italic: style.italic.unwrap_or(self.italic),
        }
    }
}

/// Appends Bedrock formatting code transitions between two style states.
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

    // A color code in Bedrock clears active formatting flags, just like in Java.
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

/// Rewrites literal text by removing Java formatting codes that have conflicting
/// meanings in Bedrock Edition (`§m` for strikethrough, `§n` for underline).
///
/// In Bedrock, `§m` is Redstone color and `§n` is Copper color.
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

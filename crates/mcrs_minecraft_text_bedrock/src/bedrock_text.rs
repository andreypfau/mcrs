use mcrs_minecraft_lang::{Piece, fill_pattern, primitive_text};
use mcrs_minecraft_text::{Text, TextContent, TextTypes, TranslateArg};

use crate::drops::BedrockDrops;
use crate::formatting::{BedrockStyle, rewrite_literal_text, write_bedrock_change};
use crate::rawtext::{BedrockRawtext, BedrockRawtextComponent, BedrockRawtextWith};
use crate::resolver::BedrockServerResolver;

/// One unit for each component visited and one for each byte emitted. A pattern can name one
/// argument many times, so nested translations grow without this bound.
const MAX_BUDGET: usize = 262_144;

/// Result of converting canonical text to Bedrock representation.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum BedrockText {
    /// Formatted string, used when no part needs localization on the client.
    String(String),
    /// Rawtext, used when a translation must stay localized on the client.
    Rawtext(BedrockRawtext),
}

impl BedrockText {
    /// Serializes to a Bedrock `rawtext` JSON string.
    pub fn to_json_string(&self) -> Result<String, serde_json::Error> {
        match self {
            Self::String(s) => serde_json::to_string(&BedrockRawtext {
                rawtext: vec![BedrockRawtextComponent::Text { text: s.clone() }],
            }),
            Self::Rawtext(r) => serde_json::to_string(r),
        }
    }
}

/// Checks if a text component or any of its children contains a translation that must stay
/// localized.
#[must_use]
pub fn has_translate<I: TextTypes>(text: &Text<I>) -> bool {
    match &text.content {
        TextContent::Translate { .. } => return true,
        TextContent::Object {
            fallback: Some(fallback),
            ..
        } if has_translate(fallback) => return true,
        _ => {}
    }
    text.extra.iter().any(has_translate)
}

/// Converts a canonical text component into either a Bedrock formatted string or rawtext,
/// resolving server-side dynamic parts and dropping unsupported styling features.
pub fn to_bedrock<I: TextTypes, R: BedrockServerResolver>(
    text: &Text<I>,
    resolver: &R,
) -> (BedrockText, BedrockDrops) {
    if has_translate(text) {
        let (rawtext, drops) = to_bedrock_rawtext(text, resolver);
        (BedrockText::Rawtext(rawtext), drops)
    } else {
        let (s, drops) = to_bedrock_string(text, resolver);
        (BedrockText::String(s), drops)
    }
}

/// Converts a canonical text component into a Bedrock formatted string. A translation shows
/// its fallback with the arguments filled in, or else its key followed by the arguments.
pub fn to_bedrock_string<I: TextTypes, R: BedrockServerResolver>(
    text: &Text<I>,
    resolver: &R,
) -> (String, BedrockDrops) {
    let mut drops = BedrockDrops::default();
    let mut walk = StringWalk {
        resolver,
        drops: &mut drops,
        out: String::new(),
        current_style: BedrockStyle::default(),
        budget: MAX_BUDGET,
    };
    walk.walk_text(text, BedrockStyle::default());
    write_bedrock_change(&mut walk.out, walk.current_style, BedrockStyle::default());
    (walk.out, drops)
}

/// Converts a canonical text component into a Bedrock rawtext structure.
pub fn to_bedrock_rawtext<I: TextTypes, R: BedrockServerResolver>(
    text: &Text<I>,
    resolver: &R,
) -> (BedrockRawtext, BedrockDrops) {
    let mut drops = BedrockDrops::default();
    let mut walk = RawtextWalk {
        resolver,
        drops: &mut drops,
        components: Vec::new(),
        current_style: BedrockStyle::default(),
        budget: MAX_BUDGET,
    };
    walk.walk_text(text, BedrockStyle::default());
    walk.restore(BedrockStyle::default());
    let rawtext = walk.components;
    (BedrockRawtext { rawtext }, drops)
}

fn primitive_arg<I: TextTypes>(arg: &TranslateArg<I>) -> Option<String> {
    match arg {
        TranslateArg::Bool(b) => Some(b.to_string()),
        TranslateArg::Number(number) => Some(primitive_text(number)),
        TranslateArg::Text(_) => None,
    }
}

fn dynamic_text<I: TextTypes, R: BedrockServerResolver>(
    resolver: &R,
    content: &TextContent<I>,
) -> Option<String> {
    match content {
        TextContent::Keybind { keybind } => Some(
            resolver
                .resolve_keybind(keybind)
                .unwrap_or_else(|| keybind.to_string()),
        ),
        TextContent::EntityNames { selector, .. } => Some(
            resolver
                .resolve_selector(selector)
                .unwrap_or_else(|| selector.to_string()),
        ),
        TextContent::Nbt { nbt, source, .. } => Some(
            resolver
                .resolve_nbt(nbt, source)
                .unwrap_or_else(|| nbt.to_string()),
        ),
        TextContent::ScoreboardValue { score } => {
            resolver.resolve_score(&score.name, &score.objective)
        }
        _ => None,
    }
}

/// Takes the part of `s` that fits the budget, with the codes that mean other things on
/// Bedrock removed.
fn take_run(s: &str, budget: &mut usize, drops: &mut BedrockDrops) -> Option<String> {
    let run = rewrite_literal_text(s, drops);
    let end = run.floor_char_boundary(*budget);
    if end < run.len() {
        drops.truncated = true;
    }
    let run = run.get(..end).filter(|run| !run.is_empty())?;
    *budget = budget.saturating_sub(run.len());
    Some(run.to_owned())
}

fn spend_node(budget: &mut usize, drops: &mut BedrockDrops) -> bool {
    match budget.checked_sub(1) {
        Some(rest) => {
            *budget = rest;
            true
        }
        None => {
            drops.truncated = true;
            false
        }
    }
}

struct StringWalk<'a, R> {
    resolver: &'a R,
    drops: &'a mut BedrockDrops,
    out: String,
    current_style: BedrockStyle,
    budget: usize,
}

impl<R: BedrockServerResolver> StringWalk<'_, R> {
    fn walk_text<I: TextTypes>(&mut self, text: &Text<I>, inherited: BedrockStyle) {
        if !spend_node(&mut self.budget, self.drops) {
            return;
        }
        let style = inherited.update_with(&text.style, self.drops);

        match &text.content {
            TextContent::Text { text } => self.emit_str(text, style),
            TextContent::Translate {
                fallback: Some(fallback),
                with,
                ..
            } => {
                fill_pattern(fallback.as_ref(), |piece| match piece {
                    Piece::Literal(literal) => self.emit_str(literal, style),
                    Piece::Argument(index) => {
                        if let Some(arg) = with.get(index) {
                            self.arg(arg, style);
                        }
                    }
                });
            }
            TextContent::Translate {
                translate, with, ..
            } => {
                let resolver = self.resolver;
                self.emit_str(resolver.translation_key(translate), style);
                for arg in with {
                    self.out.push(' ');
                    self.arg(arg, style);
                }
            }
            TextContent::Object { fallback, .. } => {
                if let Some(fallback) = fallback {
                    self.walk_text(fallback, style);
                }
            }
            content => {
                if let Some(resolved) = dynamic_text(self.resolver, content) {
                    self.emit_str(&resolved, style);
                }
            }
        }

        for child in &text.extra {
            self.walk_text(child, style);
        }
    }

    fn arg<I: TextTypes>(&mut self, arg: &TranslateArg<I>, style: BedrockStyle) {
        match arg {
            TranslateArg::Text(text) => self.walk_text(text, style),
            primitive => {
                if let Some(s) = primitive_arg(primitive) {
                    self.emit_str(&s, style);
                }
            }
        }
    }

    fn emit_str(&mut self, s: &str, style: BedrockStyle) {
        let Some(run) = take_run(s, &mut self.budget, self.drops) else {
            return;
        };
        write_bedrock_change(&mut self.out, self.current_style, style);
        self.current_style = style;
        self.out.push_str(&run);
    }
}

struct RawtextWalk<'a, R> {
    resolver: &'a R,
    drops: &'a mut BedrockDrops,
    components: Vec<BedrockRawtextComponent>,
    current_style: BedrockStyle,
    budget: usize,
}

impl<R: BedrockServerResolver> RawtextWalk<'_, R> {
    fn walk_text<I: TextTypes>(&mut self, text: &Text<I>, inherited: BedrockStyle) {
        if !spend_node(&mut self.budget, self.drops) {
            return;
        }
        let style = inherited.update_with(&text.style, self.drops);

        match &text.content {
            TextContent::Text { text } => self.emit_text(text, style),
            TextContent::Translate {
                translate, with, ..
            } => {
                let translate = self.resolver.translation_key(translate).to_owned();
                self.restore(style);
                let with = self.translate_args(with, style);
                self.components
                    .push(BedrockRawtextComponent::Translate { translate, with });
            }
            TextContent::Object { fallback, .. } => {
                if let Some(fallback) = fallback {
                    self.walk_text(fallback, style);
                }
            }
            content => {
                if let Some(resolved) = dynamic_text(self.resolver, content) {
                    self.emit_text(&resolved, style);
                }
            }
        }

        for child in &text.extra {
            self.walk_text(child, style);
        }
    }

    fn restore(&mut self, style: BedrockStyle) {
        let mut change = String::new();
        write_bedrock_change(&mut change, self.current_style, style);
        self.current_style = style;
        if !change.is_empty() {
            self.components
                .push(BedrockRawtextComponent::Text { text: change });
        }
    }

    fn emit_text(&mut self, s: &str, style: BedrockStyle) {
        let Some(run) = take_run(s, &mut self.budget, self.drops) else {
            return;
        };
        let mut text = String::new();
        write_bedrock_change(&mut text, self.current_style, style);
        self.current_style = style;
        text.push_str(&run);
        self.components.push(BedrockRawtextComponent::Text { text });
    }

    fn translate_args<I: TextTypes>(
        &mut self,
        with: &[TranslateArg<I>],
        style: BedrockStyle,
    ) -> Option<BedrockRawtextWith> {
        if with.is_empty() {
            return None;
        }
        let nested = with
            .iter()
            .any(|arg| matches!(arg, TranslateArg::Text(t) if has_translate(t)));
        if nested {
            let rawtext = with
                .iter()
                .map(|arg| match arg {
                    TranslateArg::Text(text) if has_translate(text) => {
                        self.rawtext_arg(text, style)
                    }
                    TranslateArg::Text(text) => BedrockRawtextComponent::Text {
                        text: self.string_arg(text, style),
                    },
                    primitive => BedrockRawtextComponent::Text {
                        text: primitive_arg(primitive).unwrap_or_default(),
                    },
                })
                .collect();
            Some(BedrockRawtextWith::Rawtext { rawtext })
        } else {
            let strings = with
                .iter()
                .map(|arg| match arg {
                    TranslateArg::Text(text) => self.string_arg(text, style),
                    primitive => primitive_arg(primitive).unwrap_or_default(),
                })
                .collect();
            Some(BedrockRawtextWith::Strings(strings))
        }
    }

    // A substituted argument shares the formatting state of the surrounding
    // translation, so it starts in the parent style and returns to it.
    fn string_arg<I: TextTypes>(&mut self, arg: &Text<I>, style: BedrockStyle) -> String {
        let mut walk = StringWalk {
            resolver: self.resolver,
            drops: self.drops,
            out: String::new(),
            current_style: style,
            budget: self.budget,
        };
        walk.walk_text(arg, style);
        write_bedrock_change(&mut walk.out, walk.current_style, style);
        self.budget = walk.budget;
        walk.out
    }

    // Each element of `with.rawtext` is one positional argument, so an argument
    // of several components is grouped into one nested `rawtext`.
    fn rawtext_arg<I: TextTypes>(
        &mut self,
        arg: &Text<I>,
        style: BedrockStyle,
    ) -> BedrockRawtextComponent {
        let mut walk = RawtextWalk {
            resolver: self.resolver,
            drops: self.drops,
            components: Vec::new(),
            current_style: style,
            budget: self.budget,
        };
        walk.walk_text(arg, style);
        walk.restore(style);
        self.budget = walk.budget;
        let mut components = walk.components;
        match components.pop() {
            Some(only) if components.is_empty() => only,
            last => {
                components.extend(last);
                BedrockRawtextComponent::Rawtext {
                    rawtext: components,
                }
            }
        }
    }
}

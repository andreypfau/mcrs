use mcrs_minecraft_lang::{FormatError, Piece, primitive_text, template_pieces};
use mcrs_minecraft_text::{ResolvedStyle, Text, TextContent, TextTypes, TranslateArg};

use crate::drops::BedrockDrops;
use crate::formatting::{BedrockStyle, rewrite_literal_text, write_bedrock_change};
use crate::rawtext::{BedrockRawtext, BedrockRawtextComponent, BedrockRawtextWith};
use crate::resolver::BedrockServerResolver;

const MAX_BUDGET: usize = 262_144;

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum BedrockText {
    String(String),
    Rawtext(BedrockRawtext),
}

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
    walk.walk_text(text, ResolvedStyle::default());
    write_bedrock_change(&mut walk.out, walk.current_style, BedrockStyle::default());
    (walk.out, drops)
}

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
    walk.walk_text(text, ResolvedStyle::default());
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
    fn translated<'a, I: TextTypes>(
        &mut self,
        template: &str,
        with: &'a [TranslateArg<I>],
        view: ResolvedStyle<'a, I>,
        style: BedrockStyle,
    ) {
        let pieces = match template_pieces(template, with.len()) {
            Ok(pieces) => pieces,
            Err(FormatError) => {
                self.emit_str(template, style);
                return;
            }
        };
        for piece in pieces {
            match piece {
                Piece::Literal(literal) => self.emit_str(literal, style),
                Piece::Argument(index) => {
                    if let Some(arg) = with.get(index) {
                        self.arg(arg, view);
                    }
                }
            }
        }
    }

    fn walk_text<'a, I: TextTypes>(&mut self, text: &'a Text<I>, inherited: ResolvedStyle<'a, I>) {
        if !spend_node(&mut self.budget, self.drops) {
            return;
        }
        self.drops.count(text.style.declared());
        let view = text.style.resolve(inherited);
        let style = BedrockStyle::from(view);

        match &text.content {
            TextContent::Text { text } => self.emit_str(text, style),
            TextContent::Translate {
                fallback: Some(fallback),
                with,
                ..
            } => {
                let template: &str = fallback.as_ref();
                self.translated(template, with, view, style);
            }
            TextContent::Translate {
                translate, with, ..
            } => {
                let resolver = self.resolver;
                self.emit_str(resolver.translation_key(translate), style);
                for arg in with {
                    self.out.push(' ');
                    self.arg(arg, view);
                }
            }
            TextContent::Object { fallback, .. } => {
                if let Some(fallback) = fallback {
                    self.walk_text(fallback, view);
                }
            }
            content => {
                if let Some(resolved) = dynamic_text(self.resolver, content) {
                    self.emit_str(&resolved, style);
                }
            }
        }

        for child in &text.extra {
            self.walk_text(child, view);
        }
    }

    fn arg<'a, I: TextTypes>(&mut self, arg: &'a TranslateArg<I>, view: ResolvedStyle<'a, I>) {
        match arg {
            TranslateArg::Text(text) => self.walk_text(text, view),
            primitive => {
                if let Some(s) = primitive_arg(primitive) {
                    self.emit_str(&s, BedrockStyle::from(view));
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
    fn walk_text<'a, I: TextTypes>(&mut self, text: &'a Text<I>, inherited: ResolvedStyle<'a, I>) {
        if !spend_node(&mut self.budget, self.drops) {
            return;
        }
        self.drops.count(text.style.declared());
        let view = text.style.resolve(inherited);
        let style = BedrockStyle::from(view);

        match &text.content {
            TextContent::Text { text } => self.emit_text(text, style),
            TextContent::Translate {
                translate, with, ..
            } => {
                let translate = self.resolver.translation_key(translate).to_owned();
                self.restore(style);
                let with = self.translate_args(with, view);
                self.components
                    .push(BedrockRawtextComponent::Translate { translate, with });
            }
            TextContent::Object { fallback, .. } => {
                if let Some(fallback) = fallback {
                    self.walk_text(fallback, view);
                }
            }
            content => {
                if let Some(resolved) = dynamic_text(self.resolver, content) {
                    self.emit_text(&resolved, style);
                }
            }
        }

        for child in &text.extra {
            self.walk_text(child, view);
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

    fn translate_args<'a, I: TextTypes>(
        &mut self,
        with: &'a [TranslateArg<I>],
        view: ResolvedStyle<'a, I>,
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
                    TranslateArg::Text(text) if has_translate(text) => self.rawtext_arg(text, view),
                    TranslateArg::Text(text) => BedrockRawtextComponent::Text {
                        text: self.string_arg(text, view),
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
                    TranslateArg::Text(text) => self.string_arg(text, view),
                    primitive => primitive_arg(primitive).unwrap_or_default(),
                })
                .collect();
            Some(BedrockRawtextWith::Strings(strings))
        }
    }

    fn string_arg<'a, I: TextTypes>(
        &mut self,
        arg: &'a Text<I>,
        view: ResolvedStyle<'a, I>,
    ) -> String {
        let style = BedrockStyle::from(view);
        let mut walk = StringWalk {
            resolver: self.resolver,
            drops: self.drops,
            out: String::new(),
            current_style: style,
            budget: self.budget,
        };
        walk.walk_text(arg, view);
        write_bedrock_change(&mut walk.out, walk.current_style, style);
        self.budget = walk.budget;
        walk.out
    }

    fn rawtext_arg<'a, I: TextTypes>(
        &mut self,
        arg: &'a Text<I>,
        view: ResolvedStyle<'a, I>,
    ) -> BedrockRawtextComponent {
        let style = BedrockStyle::from(view);
        let mut walk = RawtextWalk {
            resolver: self.resolver,
            drops: self.drops,
            components: Vec::new(),
            current_style: style,
            budget: self.budget,
        };
        walk.walk_text(arg, view);
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

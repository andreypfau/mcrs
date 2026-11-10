use mcrs_minecraft_lang::{Language, Piece};
use mcrs_minecraft_text::{Text, TextContent, TextTypes, TranslateArg};

use crate::RunStyle;

/// The work of one [`flatten`]: one unit for each component it visits and one for each byte it
/// emits. A pattern can name one argument many times, so nested translations grow without this
/// bound.
pub(crate) const MAX_FLAT_LEN: usize = 262_144;

/// Calls `emit` with each run of `text`, in reading order. A translation shows the pattern of
/// its key in `language`, or its fallback when the language has no such key, with the arguments
/// filled in. The walk stops after 262144 components and bytes. A keybind, a selector and an NBT path show as written; a score shows nothing.
pub fn flatten<I: TextTypes>(
    language: &Language,
    text: &Text<I>,
    mut emit: impl FnMut(&str, RunStyle),
) {
    let mut walk = Walk {
        language,
        emit: &mut emit,
        budget: MAX_FLAT_LEN,
    };
    walk.text(text, RunStyle::default());
}

struct Walk<'a, F> {
    language: &'a Language,
    emit: &'a mut F,
    budget: usize,
}

impl<F: FnMut(&str, RunStyle)> Walk<'_, F> {
    fn text<I: TextTypes>(&mut self, text: &Text<I>, inherited: RunStyle) {
        let Some(budget) = self.budget.checked_sub(1) else {
            return;
        };
        self.budget = budget;
        let style = inherited.with(&text.style);
        match &text.content {
            TextContent::Text { text } => self.run(text, style),
            TextContent::Translate {
                translate,
                fallback,
                with,
            } => {
                let pattern = match fallback {
                    Some(fallback) if !self.language.contains(translate) => fallback.as_ref(),
                    _ => self.language.get(translate),
                };
                mcrs_minecraft_lang::fill_pattern(pattern, |piece| match piece {
                    Piece::Literal(literal) => self.run(literal, style),
                    Piece::Argument(index) => {
                        if let Some(arg) = with.get(index) {
                            self.arg(arg, style);
                        }
                    }
                });
            }
            TextContent::Keybind { keybind } => self.run(keybind, style),
            TextContent::EntityNames { selector, .. } => self.run(selector, style),
            TextContent::Nbt { nbt, .. } => self.run(nbt, style),
            TextContent::Object { fallback, .. } => {
                if let Some(fallback) = fallback {
                    self.text(fallback, style);
                }
            }
            TextContent::ScoreboardValue { .. } => {}
        }
        for child in &text.extra {
            self.text(child, style);
        }
    }

    fn arg<I: TextTypes>(&mut self, arg: &TranslateArg<I>, style: RunStyle) {
        match arg {
            TranslateArg::Bool(value) => self.run(if *value { "true" } else { "false" }, style),
            TranslateArg::Number(number) => {
                self.run(&mcrs_minecraft_lang::primitive_text(number), style);
            }
            TranslateArg::Text(text) => self.text(text, style),
        }
    }

    fn run(&mut self, run: &str, style: RunStyle) {
        let run = run
            .get(..run.floor_char_boundary(self.budget))
            .unwrap_or_default();
        if run.is_empty() {
            return;
        }
        self.budget -= run.len();
        (self.emit)(run, style);
    }
}

#[cfg(test)]
mod tests {
    use mcrs_minecraft_text::ItemHover;

    use super::*;

    #[test]
    fn nested_translations_stop_at_the_bound() {
        let language = Language::from_json(br#"{"grow":"%1$s%1$s%1$s%1$s"}"#).expect("valid JSON");
        for (leaf, least) in [("ab€", MAX_FLAT_LEN / 2), ("", 0)] {
            let mut text: Text<ItemHover> = Text::text(leaf);
            for _ in 0..16 {
                text = Text::translate_args("grow", vec![text.into()]);
            }
            let mut len = 0;
            flatten(&language, &text, |run, _| len += run.len());
            assert!(
                (least..=MAX_FLAT_LEN).contains(&len),
                "{leaf:?}: {len} bytes of the 4^16 leaves stay"
            );
        }
    }
}

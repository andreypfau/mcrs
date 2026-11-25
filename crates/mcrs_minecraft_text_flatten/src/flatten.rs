use mcrs_minecraft_lang::{Language, Piece};
use mcrs_minecraft_text::{ResolvedStyle, Text, TextContent, TextTypes, TranslateArg};

pub(crate) const MAX_FLAT_LEN: usize = 262_144;

pub fn flatten<'a, I: TextTypes>(
    language: &Language,
    text: &'a Text<I>,
    mut emit: impl FnMut(&str, ResolvedStyle<'a, I>),
) {
    let mut walk = Walk {
        language,
        emit: &mut emit,
        budget: MAX_FLAT_LEN,
    };
    walk.text(text, ResolvedStyle::default());
}

struct Walk<'l, F> {
    language: &'l Language,
    emit: &'l mut F,
    budget: usize,
}

impl<F> Walk<'_, F> {
    fn text<'a, I: TextTypes>(&mut self, text: &'a Text<I>, inherited: ResolvedStyle<'a, I>)
    where
        F: FnMut(&str, ResolvedStyle<'a, I>),
    {
        let Some(budget) = self.budget.checked_sub(1) else {
            return;
        };
        self.budget = budget;
        let style = text.style.resolve(inherited);
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

    fn arg<'a, I: TextTypes>(&mut self, arg: &'a TranslateArg<I>, style: ResolvedStyle<'a, I>)
    where
        F: FnMut(&str, ResolvedStyle<'a, I>),
    {
        match arg {
            TranslateArg::Bool(value) => self.run(if *value { "true" } else { "false" }, style),
            TranslateArg::Number(number) => {
                self.run(&mcrs_minecraft_lang::primitive_text(number), style);
            }
            TranslateArg::Text(text) => self.text(text, style),
        }
    }

    fn run<'a, I: TextTypes>(&mut self, run: &str, style: ResolvedStyle<'a, I>)
    where
        F: FnMut(&str, ResolvedStyle<'a, I>),
    {
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

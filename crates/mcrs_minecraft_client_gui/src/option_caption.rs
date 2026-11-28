use mcrs_minecraft_font::font::Span;
use mcrs_minecraft_font::language::ClientLanguage;

pub(crate) fn option_caption(
    language: &ClientLanguage,
    pattern_key: &str,
    name_key: &str,
    value: &str,
) -> String {
    let name = vec![Span::new(language.get(name_key), 0)];
    language
        .translate(pattern_key, 0, &[name, vec![Span::new(value, 0)]])
        .into_iter()
        .map(|span| span.text)
        .collect()
}

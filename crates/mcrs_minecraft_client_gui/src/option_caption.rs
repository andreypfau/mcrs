use crate::font::Span;
use crate::language::ClientLanguage;

/// The caption of an option widget: the pattern `pattern_key`, for example
/// `options.generic_value`, filled with the name `name_key` and the value text `value`
/// (WID-CYC-001).
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

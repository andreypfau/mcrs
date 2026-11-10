use mcrs_minecraft_lang::Language;
use mcrs_minecraft_text::{Text, TextTypes};

// chisle: no language at this layer, so a translated reason reads as its fallback or its key;
// carry the reason as `Text` to the application when a caller needs it in a language.
pub(crate) fn disconnect_reason<I: TextTypes>(reason: &Text<I>) -> String {
    mcrs_minecraft_text_flatten::plain_text(&Language::default(), reason)
}

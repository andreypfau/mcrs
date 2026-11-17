use mcrs_minecraft_lang::Language;
use mcrs_minecraft_text::{Text, TextTypes};

pub(crate) fn disconnect_reason<I: TextTypes>(reason: &Text<I>) -> String {
    mcrs_minecraft_text_flatten::plain_text(&Language::default(), reason)
}

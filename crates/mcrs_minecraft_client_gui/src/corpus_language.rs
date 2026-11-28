use mcrs_minecraft_font::language::ClientLanguage;

use crate::corpus_pack::corpus_pack;

pub(crate) fn corpus_language() -> &'static ClientLanguage {
    static LANGUAGE: std::sync::LazyLock<ClientLanguage> =
        std::sync::LazyLock::new(|| ClientLanguage::load(corpus_pack()).unwrap());
    &LANGUAGE
}

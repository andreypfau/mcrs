use mcrs_minecraft_font::ClientLanguage;

use mcrs_minecraft_model::resource_set::ResourceSet;

pub(crate) fn corpus_language() -> &'static ClientLanguage {
    static LANGUAGE: std::sync::LazyLock<ClientLanguage> =
        std::sync::LazyLock::new(|| ClientLanguage::load(ResourceSet::corpus().files()).unwrap());
    &LANGUAGE
}

use mcrs_minecraft_font::{Font, FontOptions};

use crate::inflate_zip::inflate_zip;
use mcrs_minecraft_model::resource_set::ResourceSet;

pub(crate) fn corpus_font() -> &'static Font {
    static FONT: std::sync::LazyLock<Font> = std::sync::LazyLock::new(|| {
        Font::load(
            ResourceSet::corpus().files(),
            FontOptions::default(),
            inflate_zip,
        )
        .unwrap()
        .font
    });
    &FONT
}

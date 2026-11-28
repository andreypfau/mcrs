use mcrs_minecraft_font::font::Font;
use mcrs_minecraft_font::font_definition::FontOptions;

use crate::corpus_pack::corpus_pack;
use crate::inflate_zip::inflate_zip;

pub(crate) fn corpus_font() -> &'static Font {
    static FONT: std::sync::LazyLock<Font> = std::sync::LazyLock::new(|| {
        Font::load(corpus_pack(), FontOptions::default(), inflate_zip)
            .unwrap()
            .font
    });
    &FONT
}

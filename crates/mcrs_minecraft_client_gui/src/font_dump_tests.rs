//! The default font against the dump of the game, `tests/fixtures/vanilla/font.json`.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use crate::font::{Font, Span};
use crate::font_definition::{FontOptions, Toggle};
use crate::font_glyph::ImageSource;
use mcrs_minecraft_model::resource_set::ResourceSet;
use serde::Deserialize;

#[derive(Deserialize)]
struct Dump {
    configs: Vec<Config>,
}

#[derive(Deserialize)]
struct Config {
    options: Vec<String>,
    providers: Vec<Provider>,
    fallback_index: usize,
    glyphs: Vec<Entry>,
    #[serde(default)]
    winners: Vec<String>,
    #[serde(default)]
    bitmap_advances: BTreeMap<usize, Vec<String>>,
    #[serde(default)]
    unihex_widths: Vec<UnihexWidths>,
    #[serde(default)]
    strings: Vec<Measured>,
}

#[derive(Deserialize)]
struct Provider {
    index: usize,
    kind: String,
}

#[derive(Deserialize)]
struct Entry {
    cp: String,
    provider: usize,
    advance: f32,
    advance_bold: f32,
    bold_offset: f32,
    shadow_offset: f32,
    pixels: Option<[u32; 2]>,
    r#box: Option<[f32; 4]>,
}

#[derive(Deserialize)]
struct UnihexWidths {
    widths: Vec<WidthCount>,
}

#[derive(Deserialize)]
struct WidthCount {
    pixel_width: i32,
    count: usize,
    first: String,
}

#[derive(Deserialize)]
struct Measured {
    style: String,
    text: String,
    sum: f32,
    width: i32,
    string_width: Option<i32>,
    #[serde(default)]
    runs: Vec<Run>,
}

#[derive(Deserialize)]
struct Run {
    text: String,
    bold: bool,
}

static DUMP: LazyLock<Dump> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../tests/fixtures/vanilla/font.json")).unwrap()
});

static FONTS: LazyLock<[Font; 3]> = LazyLock::new(|| {
    let load =
        |uniform, jp| Font::load(ResourceSet::corpus(), FontOptions { uniform, jp }).unwrap();
    [
        load(Toggle::Off, Toggle::Off),
        load(Toggle::On, Toggle::Off),
        load(Toggle::Off, Toggle::On),
    ]
});

/// The font of a configuration; the font `minecraft:uniform` is the default one with `uniform`.
fn font_of(config: usize) -> &'static Font {
    match config {
        0 => &FONTS[0],
        1 | 3 => &FONTS[1],
        _ => &FONTS[2],
    }
}

fn code_point(text: &str) -> char {
    let digits = text.strip_prefix("U+").unwrap_or(text);
    let value = u32::from_str_radix(digits, 16).unwrap();
    char::from_u32(value).unwrap_or('\u{FFFD}')
}

fn text_of(code_points: &str) -> String {
    code_points.split_whitespace().map(code_point).collect()
}

#[derive(Debug, PartialEq)]
enum Class {
    Space,
    Bitmap(usize),
    Unifont,
    Missing,
}

/// Which provider a glyph came from, named by the kind and the place among the bitmap providers.
fn class_of(font: &Font, ch: char) -> Class {
    match font.glyph(ch).image.map(|image| image.source) {
        None => Class::Space,
        Some(ImageSource::Bitmap { source, .. }) => Class::Bitmap(
            font.bitmap_pages()
                .position(|(index, _)| index == source)
                .unwrap(),
        ),
        Some(ImageSource::Unifont { .. }) => Class::Unifont,
        Some(ImageSource::Missing) => Class::Missing,
    }
}

fn expected_class(config: &Config, index: usize) -> Class {
    if index == config.fallback_index {
        return Class::Missing;
    }
    let provider = config.providers.iter().find(|p| p.index == index).unwrap();
    match provider.kind.as_str() {
        "space" => Class::Space,
        "unihex" => Class::Unifont,
        "bitmap" => Class::Bitmap(
            config
                .providers
                .iter()
                .filter(|p| p.kind == "bitmap" && p.index < index)
                .count(),
        ),
        other => panic!("{other}"),
    }
}

/// FONT-PROV-002: the options of a configuration are the options of its font.
#[test]
fn the_options_of_each_configuration_load_its_font() {
    for (at, config) in DUMP.configs.iter().enumerate() {
        let options = font_of(at).options();
        assert_eq!(
            (options.uniform == Toggle::On, options.jp == Toggle::On),
            (
                config.options.iter().any(|o| o == "uniform") || at == 3,
                config.options.iter().any(|o| o == "jp"),
            ),
            "configuration {at}"
        );
    }
}

/// FONT-ORD-002, FONT-PROV-003, FONT-PROV-005, FONT-PROV-007, FONT-GLY-001 to 003, FONT-DRAW-001:
/// the provider, the advances, the offsets, the picture size and the box of each dumped glyph.
#[test]
fn every_dumped_glyph_has_the_provider_advances_offsets_and_box_of_the_game() {
    for (at, config) in DUMP.configs.iter().enumerate() {
        let font = font_of(at);
        for entry in &config.glyphs {
            let ch = code_point(&entry.cp);
            let surrogate = u32::from_str_radix(&entry.cp[2..], 16)
                .is_ok_and(|value| (0xD800..=0xDFFF).contains(&value));
            if surrogate {
                continue;
            }
            let glyph = font.glyph(ch);
            let name = format!("configuration {at}, {}", entry.cp);
            assert_eq!(
                class_of(font, ch),
                expected_class(config, entry.provider),
                "{name}"
            );
            assert_eq!(glyph.advance, entry.advance, "{name}");
            assert_eq!(font.advance(ch, true), entry.advance_bold, "{name}");
            assert_eq!(glyph.offset, entry.bold_offset, "{name}");
            assert_eq!(glyph.offset, entry.shadow_offset, "{name}");
            let size = glyph.image.map(|image| match image.source {
                ImageSource::Bitmap { texels, .. } => {
                    [texels.width() as u32, texels.height() as u32]
                }
                ImageSource::Unifont { columns, .. } => [columns, 16],
                ImageSource::Missing => [5, 8],
            });
            assert_eq!(size, entry.pixels, "{name}");
            let bounds = glyph
                .image
                .map(|image| [image.left, image.right, image.top, image.bottom]);
            assert_eq!(bounds, entry.r#box, "{name}");
        }
    }
}

/// FONT-ORD-002, FONT-SET-001: the winning provider of every code point of the basic plane.
#[test]
fn every_code_point_of_the_basic_plane_is_drawn_by_the_provider_the_game_picks() {
    let config = &DUMP.configs[0];
    let font = font_of(0);
    let mut checked = 0;
    for run in config.winners.iter().flat_map(|row| row.split_whitespace()) {
        let (range, index) = run.split_once(':').unwrap();
        let (first, last) = range.split_once('-').unwrap();
        let expected = expected_class(config, index.parse().unwrap());
        for value in
            u32::from_str_radix(first, 16).unwrap()..=u32::from_str_radix(last, 16).unwrap()
        {
            if let Some(ch) = char::from_u32(value) {
                assert_eq!(class_of(font, ch), expected, "U+{value:04X}");
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 65536 - 2048);
}

/// FONT-PROV-005: the advance of every glyph of each bitmap texture.
#[test]
fn every_bitmap_glyph_has_the_advance_of_the_game() {
    let config = &DUMP.configs[0];
    let font = font_of(0);
    for (index, rows) in &config.bitmap_advances {
        let ordinal = expected_class(config, *index);
        let Class::Bitmap(ordinal) = ordinal else {
            panic!("provider {index} is not a bitmap")
        };
        let (source, page) = font.bitmap_pages().nth(ordinal).unwrap();
        for item in rows.iter().flat_map(|row| row.split_whitespace()) {
            let (cp, advance) = item.split_once(':').unwrap();
            let ch = code_point(cp);
            assert_eq!(
                page.glyph(source, ch).unwrap().advance,
                advance.parse::<f32>().unwrap(),
                "provider {index}, {cp}"
            );
        }
    }
}

/// FONT-PROV-007, FONT-PROV-008: how many glyphs of the Unifont archive have each width.
#[test]
fn the_unifont_archive_has_the_glyph_widths_of_the_game() {
    let (_, archive) = font_of(0)
        .unifonts()
        .find(|(_, hex)| hex.len() > 100_000)
        .unwrap();
    assert_eq!(archive.len(), 114_432);
    let mut counts: BTreeMap<i32, (usize, u32)> = BTreeMap::new();
    for (code_point, width) in archive.widths() {
        let slot = counts.entry(width).or_insert((0, code_point));
        slot.0 += 1;
    }
    let dumped: Vec<_> = DUMP.configs[0].unihex_widths[0]
        .widths
        .iter()
        .map(|w| (w.pixel_width, (w.count, u32::from(code_point(&w.first)))))
        .collect();
    assert_eq!(counts.into_iter().collect::<Vec<_>>(), dumped);
}

/// FONT-WID-001: the width of each measured text, summed exactly and rounded up once.
#[test]
fn texts_measure_like_the_game() {
    let font = font_of(0);
    for measured in &DUMP.configs[0].strings {
        let text = text_of(&measured.text);
        let spans: Vec<Span> = match measured.style.as_str() {
            "plain" => vec![Span::new(text.as_str(), 0)],
            "bold" => vec![Span::new(text.as_str(), 0).bold()],
            "mixed" => measured
                .runs
                .iter()
                .map(|run| {
                    let span = Span::new(text_of(&run.text), 0);
                    if run.bold { span.bold() } else { span }
                })
                .collect(),
            other => panic!("{other}"),
        };
        let name = format!("{} {:?}", measured.style, text);
        if measured.style == "plain" {
            assert_eq!(font.exact_str_width(&text), measured.sum, "{name}");
            assert_eq!(Some(font.str_width(&text)), measured.string_width, "{name}");
        } else {
            assert_eq!(font.exact_width(&spans), measured.sum, "{name}");
            assert_eq!(font.width(&spans), measured.width, "{name}");
        }
    }
}

/// FONT-SET-001, FONT-SET-002: the scripts the game draws are drawn, not missing.
#[test]
fn cyrillic_greek_accented_latin_and_cjk_have_pictures() {
    let font = font_of(0);
    let missing = |ch: char| {
        matches!(
            font.glyph(ch).image.map(|i| i.source),
            Some(ImageSource::Missing)
        )
    };
    for ch in ('\u{0400}'..='\u{045F}').chain("ЁёЇїЄєІіҐґÉéÜßαΩ日本語한あアＡ😀".chars())
    {
        assert!(!missing(ch), "{ch:?}");
    }
    for ch in ['\u{E000}', '\u{FFFE}', '\u{F0000}'] {
        assert!(missing(ch), "{ch:?}");
    }
}

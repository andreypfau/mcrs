mod definition_error;
mod event_index;
mod font;
mod font_definition;
#[cfg(test)]
mod font_dump_tests;
mod font_error;
mod font_glyph;
mod font_load_failure;
mod font_notice;
mod font_unihex;
mod glyph_cache;
#[cfg(test)]
mod inflate_zip;
mod inflated_file;
mod language;
mod language_error;
mod span_events;
mod text;
mod unihex_error;

pub use definition_error::DefinitionError;
pub use font::{ClickEvent, Font, Hover, LoadedFont, Span, span_at, styled};
pub use font_definition::{BitmapPage, FontOptions, Toggle};
pub use font_error::FontError;
pub use font_glyph::{
    Glyph, GlyphImage, ImageSource, MISSING_GLYPH_SIZE, Weight, missing_glyph_pixels,
};
pub use font_load_failure::FontLoadFailure;
pub use font_notice::FontNotice;
pub use glyph_cache::{CELL, CELL_BYTES, COLUMNS, GlyphCache, ROWS, SLOTS, cell_origin};
pub use inflated_file::InflatedFile;
pub use language::ClientLanguage;
pub use language_error::LanguageError;
pub use span_events::SpanEvents;
pub use text::{Wrap, argb, row_texts, text_spans, text_spans_with_events, wrap};
pub use unihex_error::UnihexError;

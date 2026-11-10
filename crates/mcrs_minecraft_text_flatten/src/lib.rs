//! A text component as flat runs of styled text, and the strings made from the runs.

// std: mcrs_minecraft_lang and mcrs_minecraft_text need std.
mod flatten;
mod legacy_text;
mod plain_text;
mod run_style;

pub use flatten::flatten;
pub use legacy_text::legacy_text;
pub use plain_text::plain_text;
pub use run_style::RunStyle;

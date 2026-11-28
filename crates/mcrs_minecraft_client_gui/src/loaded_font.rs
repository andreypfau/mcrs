use crate::font::Font;
use crate::font_notice::FontNotice;

#[derive(Clone, Debug)]
pub struct LoadedFont {
    pub font: Font,
    pub notices: Vec<FontNotice>,
}

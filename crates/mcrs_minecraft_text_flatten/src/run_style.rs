use mcrs_minecraft_text::{Color, Style, TextTypes};

/// The style of one run: the style of its component over the styles of its ancestors.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct RunStyle {
    pub color: Option<Color>,
    pub obfuscated: bool,
    pub bold: bool,
    pub strikethrough: bool,
    pub underlined: bool,
    pub italic: bool,
}

impl RunStyle {
    pub(crate) fn with<I: TextTypes>(self, style: &Style<I>) -> Self {
        Self {
            color: style.color.or(self.color),
            obfuscated: style.obfuscated.unwrap_or(self.obfuscated),
            bold: style.bold.unwrap_or(self.bold),
            strikethrough: style.strikethrough.unwrap_or(self.strikethrough),
            underlined: style.underlined.unwrap_or(self.underlined),
            italic: style.italic.unwrap_or(self.italic),
        }
    }
}

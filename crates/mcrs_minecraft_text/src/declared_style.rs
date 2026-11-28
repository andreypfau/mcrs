#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct DeclaredStyle {
    pub click_event: bool,
    pub hover_event: bool,
    pub font: bool,
    pub strikethrough_enabled: bool,
    pub underlined_enabled: bool,
}

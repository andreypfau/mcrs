#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EditBoxLook {
    #[default]
    Form,
    Chat,
    CreativeSearch,
}

impl EditBoxLook {
    const FORM_TEXT_COLOR: u32 = 0xFFE0_E0E0;
    const SEARCH_TEXT_COLOR: u32 = 0xFFFF_FFFF;
    pub(crate) const UNEDITABLE_COLOR: u32 = 0xFF70_7070;
    pub(crate) const SUGGESTION_COLOR: u32 = 0xFF80_8080;

    #[must_use]
    pub(crate) const fn border(self) -> bool {
        matches!(self, Self::Form)
    }

    #[must_use]
    pub(crate) const fn text_color(self) -> u32 {
        match self {
            Self::Form | Self::Chat => Self::FORM_TEXT_COLOR,
            Self::CreativeSearch => Self::SEARCH_TEXT_COLOR,
        }
    }

    #[must_use]
    pub(crate) const fn invert(self) -> bool {
        !matches!(self, Self::CreativeSearch)
    }

    #[must_use]
    pub(crate) const fn can_lose_focus(self) -> bool {
        !matches!(self, Self::Chat | Self::CreativeSearch)
    }
}

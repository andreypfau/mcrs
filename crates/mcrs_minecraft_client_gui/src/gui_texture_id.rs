/// A texture of the widgets, resolved to its place in the GUI atlas at load.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GuiTextureId {
    MenuBackground,
    InworldMenuBackground,
    MenuListBackground,
    InworldMenuListBackground,
    HeaderSeparator,
    InworldHeaderSeparator,
    FooterSeparator,
    InworldFooterSeparator,
    TabHeaderBackground,
}

impl GuiTextureId {
    /// Every texture, in the order of declaration.
    pub(crate) const ALL: [Self; 9] = [
        Self::MenuBackground,
        Self::InworldMenuBackground,
        Self::MenuListBackground,
        Self::InworldMenuListBackground,
        Self::HeaderSeparator,
        Self::InworldHeaderSeparator,
        Self::FooterSeparator,
        Self::InworldFooterSeparator,
        Self::TabHeaderBackground,
    ];

    /// The path `p` of the texture `minecraft:p`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::MenuBackground => "textures/gui/menu_background.png",
            Self::InworldMenuBackground => "textures/gui/inworld_menu_background.png",
            Self::MenuListBackground => "textures/gui/menu_list_background.png",
            Self::InworldMenuListBackground => "textures/gui/inworld_menu_list_background.png",
            Self::HeaderSeparator => "textures/gui/header_separator.png",
            Self::InworldHeaderSeparator => "textures/gui/inworld_header_separator.png",
            Self::FooterSeparator => "textures/gui/footer_separator.png",
            Self::InworldFooterSeparator => "textures/gui/inworld_footer_separator.png",
            Self::TabHeaderBackground => "textures/gui/tab_header_background.png",
        }
    }
}

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
    TitleLogo,
    TitleMinceraft,
    TitleEdition,
    UnknownServer,
}

impl GuiTextureId {
    pub(crate) const ALL: [Self; 13] = [
        Self::MenuBackground,
        Self::InworldMenuBackground,
        Self::MenuListBackground,
        Self::InworldMenuListBackground,
        Self::HeaderSeparator,
        Self::InworldHeaderSeparator,
        Self::FooterSeparator,
        Self::InworldFooterSeparator,
        Self::TabHeaderBackground,
        Self::TitleLogo,
        Self::TitleMinceraft,
        Self::TitleEdition,
        Self::UnknownServer,
    ];

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
            Self::TitleLogo => "textures/gui/title/minecraft.png",
            Self::TitleMinceraft => "textures/gui/title/minceraft.png",
            Self::TitleEdition => "textures/gui/title/edition.png",
            Self::UnknownServer => "textures/misc/unknown_server.png",
        }
    }
}

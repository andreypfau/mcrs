/// A GUI sprite of the widgets, resolved to its place in the GUI atlas at load.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GuiSpriteId {
    TooltipBackground,
    TooltipFrame,
    Button,
    ButtonHighlighted,
    ButtonDisabled,
    Slider,
    SliderHighlighted,
    SliderHandle,
    SliderHandleHighlighted,
    Checkbox,
    CheckboxHighlighted,
    CheckboxSelected,
    CheckboxSelectedHighlighted,
    IconLanguage,
    IconAccessibility,
    TextField,
    TextFieldHighlighted,
    ScrollerBackground,
    Scroller,
    Tab,
    TabHighlighted,
    TabSelected,
    TabSelectedHighlighted,
}

impl GuiSpriteId {
    /// Every sprite, in the order of declaration.
    pub(crate) const ALL: [Self; 23] = [
        Self::TooltipBackground,
        Self::TooltipFrame,
        Self::Button,
        Self::ButtonHighlighted,
        Self::ButtonDisabled,
        Self::Slider,
        Self::SliderHighlighted,
        Self::SliderHandle,
        Self::SliderHandleHighlighted,
        Self::Checkbox,
        Self::CheckboxHighlighted,
        Self::CheckboxSelected,
        Self::CheckboxSelectedHighlighted,
        Self::IconLanguage,
        Self::IconAccessibility,
        Self::TextField,
        Self::TextFieldHighlighted,
        Self::ScrollerBackground,
        Self::Scroller,
        Self::Tab,
        Self::TabHighlighted,
        Self::TabSelected,
        Self::TabSelectedHighlighted,
    ];

    /// The path `p` of the sprite `minecraft:p`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::TooltipBackground => "tooltip/background",
            Self::TooltipFrame => "tooltip/frame",
            Self::Button => "widget/button",
            Self::ButtonHighlighted => "widget/button_highlighted",
            Self::ButtonDisabled => "widget/button_disabled",
            Self::Slider => "widget/slider",
            Self::SliderHighlighted => "widget/slider_highlighted",
            Self::SliderHandle => "widget/slider_handle",
            Self::SliderHandleHighlighted => "widget/slider_handle_highlighted",
            Self::Checkbox => "widget/checkbox",
            Self::CheckboxHighlighted => "widget/checkbox_highlighted",
            Self::CheckboxSelected => "widget/checkbox_selected",
            Self::CheckboxSelectedHighlighted => "widget/checkbox_selected_highlighted",
            Self::IconLanguage => "icon/language",
            Self::IconAccessibility => "icon/accessibility",
            Self::TextField => "widget/text_field",
            Self::TextFieldHighlighted => "widget/text_field_highlighted",
            Self::ScrollerBackground => "widget/scroller_background",
            Self::Scroller => "widget/scroller",
            Self::Tab => "widget/tab",
            Self::TabHighlighted => "widget/tab_highlighted",
            Self::TabSelected => "widget/tab_selected",
            Self::TabSelectedHighlighted => "widget/tab_selected_highlighted",
        }
    }
}

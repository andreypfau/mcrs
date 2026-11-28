use mcrs_minecraft_model_file::model_file::GuiLight;

use crate::icon_layer::IconLayer;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ItemIcon {
    pub(crate) layers: [Option<IconLayer>; ItemIcon::MAX_LAYERS],
    pub(crate) foil: bool,
    pub(crate) gui_light: GuiLight,
    pub(crate) dynamic: bool,
}

impl ItemIcon {
    pub const MAX_LAYERS: usize = 4;

    pub fn layers(&self) -> impl Iterator<Item = &IconLayer> {
        self.layers.iter().flatten()
    }

    #[must_use]
    pub const fn foil(&self) -> bool {
        self.foil
    }

    #[must_use]
    pub const fn gui_light(&self) -> GuiLight {
        self.gui_light
    }

    #[must_use]
    pub const fn is_dynamic(&self) -> bool {
        self.dynamic
    }
}

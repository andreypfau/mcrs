use bevy::prelude::Entity;

use crate::gui_layer::GuiLayerOrder;

#[derive(Clone, Copy)]
pub(crate) struct LayerSlot {
    pub(crate) order: GuiLayerOrder,
    pub(crate) entity: Entity,
}

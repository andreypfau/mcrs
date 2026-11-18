use mcrs_minecraft_model::item_model_bake::ItemModels;
use mcrs_minecraft_model::model_id::ModelId;

use crate::icon_quad::{IconQuad, icon_quads};

#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::resource::Resource))]
pub struct IconMeshes {
    meshes: Vec<Vec<IconQuad>>,
}

impl IconMeshes {
    #[must_use]
    pub fn build(models: &ItemModels) -> Self {
        Self {
            meshes: models
                .models()
                .map(|(_, model)| icon_quads(model).collect())
                .collect(),
        }
    }

    #[must_use]
    pub fn get(&self, model: ModelId) -> Option<&[IconQuad]> {
        self.meshes.get(model.index()?).map(Vec::as_slice)
    }
}

use mcrs_minecraft_model::item_model_bake::ItemModels;
use mcrs_minecraft_registries::Registries;

#[derive(Clone, Copy)]
pub struct IconSources<'a> {
    pub models: &'a ItemModels,
    pub registries: Option<&'a Registries>,
}

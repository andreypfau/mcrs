use bevy::ecs::system::SystemParam;
use bevy::math::IVec2;
use bevy::prelude::*;
use mcrs_minecraft_item_icon::{
    IconMeshes, IconSources, ItemIcon, Overlays, StackOverlay, resolve,
};
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_model::item_model_asset::DisplayContext;
use mcrs_minecraft_model::item_model_bake::ItemModels;
use mcrs_minecraft_model::item_model_context::ItemModelContext;
use mcrs_minecraft_model::stack_view::StackView;
use mcrs_minecraft_registries::CurrentRegistries;

use crate::scene::{GuiQuad, GuiScene, SceneIcon};

const GUI: ItemModelContext = ItemModelContext::new(DisplayContext::Gui);

#[derive(SystemParam)]
pub struct ItemIcons<'w> {
    models: Option<Res<'w, ItemModels>>,
    table: Option<Res<'w, ItemTable>>,
    registries: Option<Res<'w, CurrentRegistries>>,
    meshes: Option<Res<'w, IconMeshes>>,
}

impl ItemIcons<'_> {
    pub fn draw_stack(
        &self,
        scene: &mut GuiScene,
        origin: IVec2,
        stack: &ItemStack,
        overlays: Overlays,
    ) {
        let (Some(models), Some(table)) = (self.models.as_deref(), self.table.as_deref()) else {
            return;
        };
        let view = StackView { stack, table };
        let sources = IconSources {
            models,
            registries: self.registries.as_deref().map(|registries| &**registries),
        };
        let icon = resolve(view, &sources, &GUI);
        Self::draw_icon(scene, origin, icon, StackOverlay::of(view), overlays);
    }

    pub fn draw_icon(
        scene: &mut GuiScene,
        origin: IVec2,
        icon: ItemIcon,
        overlay: StackOverlay,
        overlays: Overlays,
    ) {
        let index = scene.icons.len();
        scene.icons.push(SceneIcon {
            icon,
            overlay,
            overlays,
        });
        scene.quads.push(GuiQuad::Icon { origin, index });
    }

    pub(crate) fn meshes(&self) -> Option<&IconMeshes> {
        self.meshes.as_deref()
    }
}

pub(crate) fn build_icon_meshes(models: Res<ItemModels>, mut commands: Commands) {
    commands.insert_resource(IconMeshes::build(&models));
}

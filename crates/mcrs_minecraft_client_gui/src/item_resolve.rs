use bevy::app::{App, Plugin, Update};
use bevy::prelude::{
    Commands, Component, DetectChanges, Entity, IntoScheduleConfigs, Local, MessageReader, Query,
    Res, SystemCondition, on_message, resource_changed, resource_exists,
};
use mcrs_minecraft_engine_item::SlotChanged;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_item::Stacks;
use mcrs_minecraft_inventory_client::InventoryClientSet;
use mcrs_minecraft_item_icon::{Durability, GuiVertex, IconSources, StackOverlay, icon_quads};
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_network::client::ClientNetworkSystems;
use mcrs_minecraft_registries::{CurrentRegistries, Registries};

use mcrs_minecraft_model::item_model_asset::DisplayContext;
use mcrs_minecraft_model::item_model_bake::ItemModels;
use mcrs_minecraft_model::item_model_context::ItemModelContext;
use mcrs_minecraft_model::stack_view::StackView;
use mcrs_minecraft_model_file::model_file::GuiLight;

#[derive(Debug)]
pub struct RenderLayer {
    pub vertices: Vec<GuiVertex>,
    pub foil: bool,
}

#[derive(Component, Debug)]
pub struct ItemRenderLayers {
    pub layers: Vec<RenderLayer>,
    pub gui_light: GuiLight,
    pub damage: Option<Durability>,
}

pub struct ItemRenderPlugin;

impl Plugin for ItemRenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            resolve_item_layers
                .after(ClientNetworkSystems::Receive)
                .after(InventoryClientSet::Predict)
                .run_if(
                    resource_exists::<ItemTable>
                        .and_then(resource_exists::<ItemModels>)
                        .and_then(
                            on_message::<SlotChanged>
                                .or_else(resource_changed::<ItemModels>)
                                .or_else(resource_changed::<ItemTable>),
                        ),
                ),
        );
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system: each access is its own parameter"
)]
pub fn resolve_item_layers(
    models: Res<ItemModels>,
    table: Res<ItemTable>,
    registries: Option<Res<CurrentRegistries>>,
    mut changes: MessageReader<SlotChanged>,
    holders: Query<&Slots>,
    mut stacks: Stacks,
    mut dirty: Local<Vec<Entity>>,
    mut commands: Commands,
) {
    dirty.clear();
    if models.is_changed() || table.is_changed() {
        changes.clear();
        for slots in &holders {
            dirty.extend(slots.occupied());
        }
    } else {
        for change in changes.read() {
            let slot = change.slot;
            let occupant = holders
                .get(slot.holder)
                .ok()
                .and_then(|slots| slots.get(usize::from(slot.index)));
            dirty.extend(occupant);
        }
    }
    dirty.sort_unstable();
    dirty.dedup();
    let registries = registries.as_deref().map(|registries| &**registries);
    for &stack in dirty.iter() {
        let Some(value) = stacks.to_value(stack) else {
            continue;
        };
        commands
            .entity(stack)
            .try_insert(resolve(&value, &table, &models, registries));
    }
}

pub fn resolve(
    stack: &ItemStack,
    table: &ItemTable,
    models: &ItemModels,
    registries: Option<&Registries>,
) -> ItemRenderLayers {
    let view = StackView { stack, table };
    let icon = mcrs_minecraft_item_icon::resolve(
        view,
        &IconSources { models, registries },
        &ItemModelContext::new(DisplayContext::Gui),
    );
    ItemRenderLayers {
        layers: icon
            .layers()
            .filter_map(|layer| {
                let model = models.model(layer.model)?;
                Some(RenderLayer {
                    vertices: icon_quads(model)
                        .flat_map(|quad| quad.tinted(&layer.tints))
                        .collect(),
                    foil: icon.foil(),
                })
            })
            .collect(),
        gui_light: icon.gui_light(),
        damage: StackOverlay::of(view).durability,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{LazyLock, OnceLock};

    use bevy::app::App;
    use bevy::prelude::DetectChanges;
    use mcrs_minecraft_engine_item::HolderCommandsExt;
    use mcrs_minecraft_engine_item::ItemCommandsExt;
    use mcrs_minecraft_engine_item::ItemPlugin;
    use mcrs_minecraft_engine_item::StackCommandsExt;
    use mcrs_minecraft_engine_registries::test_item_table;
    use mcrs_minecraft_item_stack::Count;
    use mcrs_minecraft_item_stack::ItemModel;
    use mcrs_minecraft_item_stack::Kind;
    use mcrs_minecraft_item_stack::NameLookup;
    use mcrs_minecraft_item_stack::NameTable;
    use mcrs_minecraft_resource::ResourceLocation;

    use super::*;
    use mcrs_minecraft_model::atlas::SpriteRegistry;
    use mcrs_minecraft_model::item_model_bake::bake_all;
    use mcrs_minecraft_model::resource_set::ResourceSet;

    static TABLE: LazyLock<ItemTable> = LazyLock::new(test_item_table);

    fn models() -> &'static ItemModels {
        static MODELS: OnceLock<ItemModels> = OnceLock::new();
        MODELS.get_or_init(|| {
            bake_all(ResourceSet::corpus(), &mut SpriteRegistry::default()).unwrap()
        })
    }

    fn stack(path: &str, count: u8) -> ItemStack {
        let name = format!("minecraft:{path}");
        let location = ResourceLocation::read(&name).unwrap();
        let kind = Kind(TABLE.id(NameTable::Item, location).unwrap());
        ItemStack::new(kind, Count::new(count).unwrap())
    }

    mod jar {
        use super::*;

        #[test]

        fn the_system_rebuilds_only_the_stacks_of_changed_slots() {
            let mut app = App::new();
            app.insert_resource(*TABLE)
                .insert_resource({
                    let mut models = models().clone();
                    models.grass_colormap = None;
                    models
                })
                .add_plugins((ItemPlugin::default(), ItemRenderPlugin));
            let world = app.world_mut();
            let holder = world.spawn_empty().id();
            world
                .commands()
                .entity(holder)
                .load_inventory(vec![Some(stack("stick", 1)); 3]);
            world.flush();
            app.update();
            let occupant = |app: &App, index| app.world().get::<Slots>(holder).unwrap().get(index);
            let (first, second) = (occupant(&app, 0).unwrap(), occupant(&app, 1).unwrap());
            let changed = |app: &App, stack: Entity| {
                let layers = app
                    .world()
                    .entity(stack)
                    .get_ref::<ItemRenderLayers>()
                    .unwrap();
                (layers.gui_light, layers.last_changed())
            };
            let (light, built) = changed(&app, second);
            assert_eq!(light, GuiLight::Front);
            let (_, first_built) = changed(&app, first);
            let third = occupant(&app, 2).unwrap();
            let untouched = changed(&app, third);

            app.update();
            assert_eq!(changed(&app, first), (GuiLight::Front, first_built));
            assert_eq!(changed(&app, second), (GuiLight::Front, built));

            let world = app.world_mut();
            let model = ItemModel::from_static(ResourceLocation::read("minecraft:stone").unwrap());
            world.commands().entity(first).despawn();
            world
                .commands()
                .spawn_stack(stack("stick", 1).set(model))
                .put_at(holder, 0);
            world
                .commands()
                .entity(second)
                .insert(Count::new(2).unwrap())
                .touch();
            world.flush();
            app.update();
            let replaced = occupant(&app, 0).unwrap();
            assert_ne!(replaced, first);
            assert_eq!(changed(&app, replaced).0, GuiLight::Side);
            let (light, rebuilt) = changed(&app, second);
            assert_eq!(light, GuiLight::Front);
            assert_ne!(
                rebuilt, built,
                "a count change of a slot rebuilds its stack"
            );
            assert_eq!(
                changed(&app, third),
                untouched,
                "an unchanged slot keeps its layers"
            );
        }
    }
}
